# Getting started: JavaScript

[`@openbim/ifcx`](/reference/crates/openbim-ifcx-wasm) is the WebAssembly
build of the Rust crates, for Node 18 and later, bundlers and plain browser
pages, with TypeScript declarations.

```sh
npm install @openbim/ifcx
```

The Node snippets on this page are regions of
[`crates/openbim-ifcx-wasm/tests/js/guide.mjs`](https://github.com/openbimrs/ifcx/blob/main/crates/openbim-ifcx-wasm/tests/js/guide.mjs),
which the gate runs against the built package, so they work as shown.

## Node

```js
import { readFileSync, writeFileSync } from "node:fs";
import { IfcxFile, compose, exportGlb, fetchImports, validate } from "@openbim/ifcx";
// or: const { IfcxFile, ... } = require("@openbim/ifcx");
```

### Read, validate and write

<<< ../../crates/openbim-ifcx-wasm/tests/js/guide.mjs#read{js}

Keep the `IfcxFile` and call `write()` to save a file: a plain object from
`toJSON()` may reorder integer-like keys and round integers beyond 2^53.

### Compose layers and export GLB

<<< ../../crates/openbim-ifcx-wasm/tests/js/guide.mjs#compose{js}

### Imports

Without `options.imports`, imports are not resolved. With it, every
imported file must be supplied, keyed by its exact import `uri`.
`fetchImports` collects them for you:

<<< ../../crates/openbim-ifcx-wasm/tests/js/guide.mjs#fetch{js}

`fetch` defaults to the global `fetch`; pass your own for credentials, a
cache, a mirror or local files, such as
`{ fetch: async (url) => readFile(new URL(url)) }` in Node.

## Browser

With a bundler that supports WebAssembly ES module integration (webpack 5
with `experiments.asyncWebAssembly`), import `@openbim/ifcx` as in Node.
Otherwise, and with Vite, import the `web` build and call `init()` once:

```html
<script type="importmap">
  { "imports": { "@openbim/ifcx/web": "/node_modules/@openbim/ifcx/web/openbim_ifcx_wasm.js" } }
</script>
<script type="module">
  import init, { IfcxFile, exportGlb, fetchImports, validate } from "@openbim/ifcx/web";

  await init(); // fetches openbim_ifcx_wasm_bg.wasm next to the module
  const url = new URL("models/house.ifcx", location.href);
  const model = new Uint8Array(await (await fetch(url)).arrayBuffer());

  // Fetch what the model imports, relative to its own URL, then use it.
  const imports = await fetchImports(model, { baseUrl: url });
  const report = validate(model, { imports });
  const glb = exportGlb(model, { imports }); // e.g. for three.js's GLTFLoader.parse
  console.log(IfcxFile.parse(model).header.id, report.valid, glb.length);
</script>
```

The package's own check loads this build in headless Chrome on every
change. The [viewer](/viewer/){target="_self"} is a complete example:
Vite, the `web` build, three.js, picking by the IFCX path each glTF node
carries; its source is
[`demo/`](https://github.com/openbimrs/ifcx/tree/main/demo).

## Errors

Every failure throws an `Error` with `name === "IfcxError"` and a stable
`code`: `read`, `write`, `layer`, `compose`, `invalid-argument`, `glb` or
`fetch`. The [reference](/reference/crates/openbim-ifcx-wasm) lists the
whole API, generated from the binding's declarations.
