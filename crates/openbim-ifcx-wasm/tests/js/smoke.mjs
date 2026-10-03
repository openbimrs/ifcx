// Suite for the openbim-ifcx-wasm Node package, run against the built
// package by scripts/build-node-pkg.sh.
//
// Proves the binding works from JavaScript, not just that it compiles: read
// and write back the hand-written fixtures, validate, compose layers with
// and without imports, and check every refusal throws an `IfcxError` with a
// stable `code`. The behaviour itself is tested in openbim-ifcx-binding-core.
//
// Usage: IFCX_WASM_PKG=<package-dir> node --test smoke.mjs
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const pkg = path.resolve(process.env.IFCX_WASM_PKG ?? "pkg");
const ifcx = createRequire(import.meta.url)(pkg);
const { IfcxFile, compose, exportGlb, validate } = ifcx;

const fixtures = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  "../../../openbim-ifcx/tests/fixtures",
);
const text = (name) => readFileSync(path.join(fixtures, name), "utf8");
const bytes = (name) => new Uint8Array(readFileSync(path.join(fixtures, name)));

/** `fn` must throw an IfcxError with this code. */
function throwsCode(fn, code) {
  assert.throws(fn, (error) => {
    assert.equal(error.name, "IfcxError");
    assert.equal(error.code, code, error.message);
    return true;
  });
}

/** A layer that renames the roof of geometry-model.ifcx. */
function overlay(imports = []) {
  return JSON.stringify({
    header: {
      id: "overlay", ifcxVersion: "ifcx_alpha", dataVersion: "1.0.0",
      author: "test", timestamp: "2026-10-03",
    },
    imports: imports.map((uri) => ({ uri })),
    schemas: {},
    data: [{ path: "roof", attributes: { "example::class": "Roof" } }],
  });
}

const roofClass = (tree) =>
  tree.children.pavilion.children.Storey.children.Roof.attributes["example::class"];

test("the package is the npm package the release publishes", () => {
  const manifest = JSON.parse(readFileSync(path.join(pkg, "package.json"), "utf8"));
  assert.equal(manifest.name, "@openbim/ifcx");
  for (const file of manifest.files) {
    assert.ok(readFileSync(path.join(pkg, file)).length > 0, file);
  }
});

test("a file reads from a string or bytes and writes back unchanged", () => {
  for (const name of ["minimal.ifcx", "wall-with-type.ifcx", "unknown-fields.ifcx", "geometry-model.ifcx"]) {
    const fromText = IfcxFile.parse(text(name));
    const fromBytes = IfcxFile.parse(bytes(name));
    assert.equal(fromText.write(), fromBytes.write(), name);
    assert.equal(IfcxFile.parse(fromText.write(true)).write(), fromText.write(), name);
  }
  // Exact numbers and unknown fields survive in the written text.
  const written = IfcxFile.parse(text("unknown-fields.ifcx")).write(true);
  assert.deepEqual(JSON.parse(written), JSON.parse(text("unknown-fields.ifcx")));
});

test("header, node count and plain objects", () => {
  const file = IfcxFile.parse(text("geometry-model.ifcx"));
  assert.equal(file.header.ifcxVersion, "ifcx_alpha");
  assert.equal(file.nodeCount, 9);
  const object = file.toJSON();
  assert.equal(object.data[0].path, "pavilion");
  assert.deepEqual(object, JSON.parse(file.write()));
  assert.equal(JSON.stringify(file), JSON.stringify(object));
});

test("validation is a structured report", () => {
  assert.deepEqual(IfcxFile.parse(text("valid-attributes.ifcx")).validate(), {
    valid: true,
    failures: [],
  });
  const report = IfcxFile.parse(text("invalid-attributes.ifcx")).validate();
  assert.equal(report.valid, false);
  assert.ok(report.failures.length > 3);
  for (const failure of report.failures) {
    assert.deepEqual(Object.keys(failure), ["node", "attribute", "pointer", "kind", "message"]);
    assert.notEqual(failure.kind, "other", failure.message);
  }
  assert.ok(report.failures.some((f) => f.kind === "missing-schema"));
  assert.deepEqual(validate([text("geometry-model.ifcx"), overlay()]), { valid: true, failures: [] });
});

test("compose returns the tree upstream composes", () => {
  const tree = compose([text("occurrence-type.ifcx")]);
  assert.deepEqual(tree, JSON.parse(text("occurrence-type.composed.json")));
  assert.deepEqual(compose(bytes("occurrence-type.ifcx")), tree, "a single layer");
  assert.equal(tree.path, "");
});

test("the last layer wins, and imports resolve from memory", () => {
  const model = text("geometry-model.ifcx");
  assert.equal(roofClass(compose([model])), "Slab");
  assert.equal(roofClass(compose([model, overlay()])), "Roof");
  assert.equal(roofClass(compose([overlay(), model])), "Slab");

  // In upstream order an import overrides the layer importing it.
  const imports = { "model.ifcx": bytes("geometry-model.ifcx") };
  assert.equal(roofClass(compose([overlay(["model.ifcx"])], { imports })), "Slab");
  assert.equal(
    roofClass(compose([overlay(["model.ifcx"]), overlay()], { imports: new Map(Object.entries(imports)) })),
    "Roof",
  );
  assert.equal(validate([overlay(["model.ifcx"])], { imports }).valid, true);

  // A chain with an integrity value.
  const chain = {
    "mid.ifcx": bytes("layers/chain/mid.ifcx"),
    "sub/base.ifcx": text("layers/chain/sub/base.ifcx"),
  };
  assert.equal(validate([text("layers/chain/main.ifcx")], { imports: chain }).valid, true);
});

/** The JSON chunk of a GLB file, after checking its container. */
function glbJson(glb) {
  assert.ok(glb instanceof Uint8Array);
  const view = new DataView(glb.buffer, glb.byteOffset, glb.byteLength);
  assert.equal(new TextDecoder().decode(glb.subarray(0, 4)), "glTF");
  assert.equal(view.getUint32(4, true), 2);
  assert.equal(view.getUint32(8, true), glb.length);
  const length = view.getUint32(12, true);
  assert.equal(new TextDecoder().decode(glb.subarray(16, 20)), "JSON");
  return JSON.parse(new TextDecoder().decode(glb.subarray(20, 20 + length)));
}

test("composed layers export as GLB", () => {
  const model = text("geometry-model.ifcx");
  const json = glbJson(exportGlb(model));
  const names = json.nodes.map((node) => node.name);
  assert.ok(names.includes("pavilion/Storey/Column 1/Body"), names);
  assert.ok(Array.isArray(json.nodes[0].rotation), "Y-up by default");

  const local = glbJson(exportGlb([bytes("geometry-model.ifcx")], { yUp: false, originOnRoot: false }));
  assert.equal(local.nodes[0].rotation, undefined);
  assert.equal(local.nodes[0].translation, undefined);

  const imports = { "model.ifcx": model };
  const layered = glbJson(exportGlb([overlay(["model.ifcx"]), overlay()], { imports }));
  assert.equal(layered.nodes.length, json.nodes.length);

  throwsCode(() => exportGlb(model, { yUp: "no" }), "invalid-argument");
  throwsCode(() => exportGlb([text("layer-edit.ifcx")]), "compose");
});

test("every failure is an IfcxError with a stable code", () => {
  throwsCode(() => IfcxFile.parse("not json"), "read");
  throwsCode(() => IfcxFile.parse("{}"), "read");
  throwsCode(() => IfcxFile.parse(42), "invalid-argument");
  throwsCode(() => compose([]), "invalid-argument");
  throwsCode(() => compose([text("minimal.ifcx"), 7]), "invalid-argument");
  throwsCode(() => compose([text("minimal.ifcx")], { imports: [] }), "invalid-argument");
  throwsCode(() => compose([text("minimal.ifcx")], "imports"), "invalid-argument");
  throwsCode(() => compose([text("minimal.ifcx"), "[]"]), "read");
  throwsCode(() => compose([text("layer-edit.ifcx")]), "compose");
  throwsCode(() => compose([text("layers/chain/main.ifcx")], { imports: {} }), "layer");
  throwsCode(
    () => compose([text("layers/chain/mid.ifcx")], { imports: { "sub/base.ifcx": text("minimal.ifcx") } }),
    "layer",
  );
  // An error is still an Error.
  try {
    compose([]);
  } catch (error) {
    assert.ok(error instanceof Error);
  }
});

test("README example: read, validate, compose", () => {
  const model = text("geometry-model.ifcx");
  const overlayText = overlay();
  // Mirrors the README example.
  const file = IfcxFile.parse(model); // a string or a Uint8Array
  const report = file.validate(); // { valid, failures: [{ node, attribute, pointer, kind, message }] }
  const out = file.write(); // JSON text, unchanged from the input

  // Layers weakest first: the overlay's opinions win.
  const tree = compose([model, overlayText]);
  const storey = tree.children.pavilion.children.Storey;
  assert.equal(report.valid, true);
  assert.equal(out, IfcxFile.parse(out).write());
  assert.equal(storey.children.Roof.attributes["example::class"], "Roof");
});
