// The code of the documentation site's JavaScript guide
// (docs/guide/javascript.md), run against the built package by
// scripts/build-npm-pkg.sh. The guide imports the `#region` blocks below
// verbatim, so every snippet it shows runs in the gate. Edit the code
// here, not on the page.
//
// Usage: IFCX_WASM_PKG=<package-dir> node --test guide.mjs
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

// The page shows `import { … } from "@openbim/ifcx";`.
const pkg = path.resolve(process.env.IFCX_WASM_PKG ?? "pkg");
const { IfcxFile, compose, exportGlb, fetchImports, validate } = createRequire(import.meta.url)(pkg);

const fixtures = path.join(path.dirname(fileURLToPath(import.meta.url)), "../../../openbim-ifcx/tests/fixtures");
const work = mkdtempSync(path.join(tmpdir(), "ifcx-guide-"));
process.chdir(work);
writeFileSync("model.ifcx", readFileSync(path.join(fixtures, "geometry-model.ifcx")));
writeFileSync(
  "overlay.ifcx",
  JSON.stringify({
    header: { id: "overlay", ifcxVersion: "ifcx_alpha", dataVersion: "1.0.0", author: "guide", timestamp: "2026-10-03" },
    imports: [],
    schemas: {},
    data: [],
  }),
);
test.after(() => rmSync(work, { recursive: true, force: true }));

test("read, validate and write", () => {
  // #region read
  const model = readFileSync("model.ifcx"); // a Buffer is a Uint8Array; a string works too
  const file = IfcxFile.parse(model);
  console.log(file.header.id, file.nodeCount);

  const report = file.validate(); // against the file's own schemas
  for (const failure of report.failures) {
    console.log(failure.node, failure.attribute, failure.pointer, failure.kind, failure.message);
  }

  const text = file.write(true); // JSON text: content, key order and numbers as read
  // #endregion read
  assert.equal(report.valid, true);
  assert.equal(IfcxFile.parse(text).write(true), text);
});

test("compose, validate layers and export GLB", () => {
  const model = readFileSync("model.ifcx");
  // #region compose
  // Layers weakest first: the last layer's opinions win.
  const layers = [model, readFileSync("overlay.ifcx")];

  const tree = compose(layers); // { path, attributes, children }
  console.log(Object.keys(tree.children)); // the root nodes

  const { valid, failures } = validate(layers);

  const glb = exportGlb(layers); // Uint8Array, binary glTF 2.0
  writeFileSync("model.glb", glb);
  // #endregion compose
  assert.ok(Object.keys(tree.children).length > 0);
  assert.equal(valid, true);
  assert.equal(failures.length, 0);
  assert.equal(new TextDecoder().decode(glb.subarray(0, 4)), "glTF");
});

test("fetchImports with a custom loader", async () => {
  const main = JSON.stringify({
    header: { id: "main", ifcxVersion: "ifcx_alpha", dataVersion: "1.0.0", author: "guide", timestamp: "2026-10-03" },
    imports: [{ uri: "https://example.org/model/types.ifcx" }],
    schemas: {},
    data: [{ path: "wall", inherits: { type: "panel" } }],
  });
  const served = {
    "https://example.org/model/types.ifcx": JSON.stringify({
      header: { id: "types", ifcxVersion: "ifcx_alpha", dataVersion: "1.0.0", author: "guide", timestamp: "2026-10-03" },
      imports: [],
      schemas: {},
      data: [{ path: "panel", attributes: {} }],
    }),
  };
  const fetch = async (url) => served[url];
  // #region fetch
  // Fetch every file the layers import, recursively, keyed by import `uri`.
  // The WebAssembly module never touches the network; this is plain JS
  // over `fetch` (or the loader you pass).
  const imports = await fetchImports([main], { baseUrl: "https://example.org/model/", fetch });
  const composed = compose([main], { imports });
  // #endregion fetch
  assert.ok(imports.has("https://example.org/model/types.ifcx"));
  assert.ok(composed.children.wall);
});
