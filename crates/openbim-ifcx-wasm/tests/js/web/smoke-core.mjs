// The checks every packaged target must pass (#46).
//
// Plain ECMAScript, no Node or DOM API, so the same checks run in Node, in a
// webpack bundle and in a browser page. `smoke(ifcx, host)` throws on the
// first failed check and otherwise returns what it saw, for the caller to
// report. The full Node suite (../smoke.mjs) covers the API itself; this
// proves each build loads its wasm module and works end to end on the
// repository's hand-written fixtures: parse, validate, compose, fetch
// imports and export GLB.
//
// `host.text(name)` resolves to a fixture's text, `host.base` is the URL of
// the fixtures directory, and `host.fetch` is the fetch function to resolve
// imports with (undefined: the global `fetch`).

function check(condition, what) {
  if (!condition) throw new Error(`smoke check failed: ${what}`);
}

/** JSON text with sorted keys, to compare plain objects. */
function canonical(value) {
  return JSON.stringify(value, (_, v) =>
    v !== null && typeof v === "object" && !Array.isArray(v)
      ? Object.fromEntries(Object.entries(v).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)))
      : v,
  );
}

/** `promise` must reject (or `fn` throw) with an IfcxError of this code. */
async function rejectsCode(fn, code) {
  try {
    await fn();
  } catch (error) {
    check(error.name === "IfcxError", `error name ${error.name}: ${error.message}`);
    check(error.code === code, `error code ${error.code}, wanted ${code}: ${error.message}`);
    return error.message;
  }
  throw new Error(`smoke check failed: no IfcxError ${code}`);
}

/** The JSON chunk of a GLB file, after checking its container. */
function glbJson(glb) {
  check(glb instanceof Uint8Array, "exportGlb returns a Uint8Array");
  const view = new DataView(glb.buffer, glb.byteOffset, glb.byteLength);
  check(new TextDecoder().decode(glb.subarray(0, 4)) === "glTF", "GLB magic");
  check(view.getUint32(4, true) === 2, "GLB version 2");
  check(view.getUint32(8, true) === glb.length, "GLB length");
  const length = view.getUint32(12, true);
  check(new TextDecoder().decode(glb.subarray(16, 20)) === "JSON", "GLB JSON chunk");
  return JSON.parse(new TextDecoder().decode(glb.subarray(20, 20 + length)));
}

export async function smoke(ifcx, host) {
  const { IfcxFile, compose, validate, exportGlb, fetchImports } = ifcx;
  const model = await host.text("geometry-model.ifcx");

  // Parse from text and bytes; write back losslessly.
  const file = IfcxFile.parse(model);
  check(file.nodeCount === 9, `nodeCount ${file.nodeCount}`);
  check(file.header.ifcxVersion === "ifcx_alpha", "header");
  const fromBytes = IfcxFile.parse(new TextEncoder().encode(model));
  check(fromBytes.write() === file.write(), "text and bytes read the same file");
  await rejectsCode(() => IfcxFile.parse("{"), "read");

  // Validate.
  check(file.validate().valid, "geometry-model validates");
  const invalid = IfcxFile.parse(await host.text("invalid-attributes.ifcx")).validate();
  check(!invalid.valid && invalid.failures.some((f) => f.kind === "missing-schema"), "invalid report");

  // Compose, as upstream does.
  const tree = compose([await host.text("occurrence-type.ifcx")]);
  const expected = JSON.parse(await host.text("occurrence-type.composed.json"));
  check(canonical(tree) === canonical(expected), "composed tree equals upstream's");

  // Fetch imports over the network, then compose and validate with them.
  const main = await host.text("layers/chain/main.ifcx");
  const imports = await fetchImports(main, {
    baseUrl: new URL("layers/chain/", host.base),
    fetch: host.fetch,
  });
  check(imports instanceof Map, "fetchImports returns a Map");
  const uris = [...imports.keys()].sort();
  check(uris.join() === "mid.ifcx,sub/base.ifcx", `fetched ${uris}`);
  // Validation loads every import and checks mid.ifcx's sha256 integrity
  // value against the fetched bytes of sub/base.ifcx.
  check(validate([main], { imports }).valid, "chain validates with fetched imports");
  const tampered = new Map(imports).set("sub/base.ifcx", await host.text("minimal.ifcx"));
  await rejectsCode(() => validate([main], { imports: tampered }), "layer");
  // A file that is not there rejects; nothing reaches the binding.
  const missing = await host.text("layers/missing/main.ifcx");
  await rejectsCode(
    () => fetchImports(missing, { baseUrl: new URL("layers/missing/", host.base), fetch: host.fetch }),
    "fetch",
  );

  // Export GLB.
  const glb = exportGlb(model);
  const json = glbJson(glb);
  const names = json.nodes.map((node) => node.name);
  check(names.includes("pavilion/Storey/Column 1/Body"), `GLB node names ${names}`);

  return { nodes: file.nodeCount, imports: uris.length, glbBytes: glb.length, glbNodes: names.length };
}
