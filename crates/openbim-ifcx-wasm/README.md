# @openbim/ifcx

Read, write, validate and compose **IFC5 / IFCX** files, and export them as
GLB, from JavaScript and TypeScript. A WebAssembly build of the
[`openbim-ifcx`](https://crates.io/crates/openbim-ifcx) Rust crate, from the
`openbim-ifcx-wasm` crate in [openbimrs/ifcx](https://github.com/openbimrs/ifcx).

Published to npm as `@openbim/ifcx`: one package for Node 18 and later,
bundlers (webpack, Rollup) and plain browser pages, with TypeScript
declarations for each. Every build is tested from the packed tarball,
the browser builds in headless Chrome.

Try it in the browser: the [viewer](https://openbimrs.github.io/ifcx/viewer/)
composes a file with this package and shows the GLB with three.js, on
desktop and on phones. The [documentation](https://openbimrs.github.io/ifcx/)
has a [JavaScript guide](https://openbimrs.github.io/ifcx/guide/javascript)
and the [API reference](https://openbimrs.github.io/ifcx/reference/crates/openbim-ifcx-wasm).

Targets the `ifcx_alpha` draft of buildingSMART's IFC5. IFCX is still a
moving draft; see the
[capabilities](https://github.com/openbimrs/ifcx/blob/main/docs/capabilities.md).

## Install

```sh
npm install @openbim/ifcx
```

## Entry points

| Import | Build | Loads the wasm module |
| --- | --- | --- |
| `@openbim/ifcx` in Node (`require` or `import`) | CommonJS (`wasm-bindgen --target nodejs`) | synchronously, on load |
| `@openbim/ifcx` in a bundler | ES module (`--target bundler`) | through the bundler's WebAssembly support, e.g. webpack 5 `experiments.asyncWebAssembly` |
| `@openbim/ifcx/web` | ES module (`--target web`) | when you `await init()` |

`package.json` `exports` picks the build: the `node` condition gets the
CommonJS build, everything else the bundler build. Use `@openbim/ifcx/web`
for a page without a bundler, and for bundlers without WebAssembly ES
module integration, such as Vite (or add `vite-plugin-wasm`). All three
export the same API; the `web` build adds the default `init` export (and
`initSync`).

## Node

```js
const { readFileSync, writeFileSync } = require("node:fs");
const { IfcxFile, compose, exportGlb } = require("@openbim/ifcx"); // or import

const model = readFileSync("model.ifcx"); // a Buffer is a Uint8Array
const file = IfcxFile.parse(model); // or a string
console.log(file.header.id, file.nodeCount);

const report = file.validate();
for (const f of report.failures) {
  console.log(f.node, f.attribute, f.pointer, f.kind, f.message);
}
const text = file.write(); // JSON text, unchanged from the input

// Layers weakest first: the last layer's opinions win.
const tree = compose([model, readFileSync("overlay.ifcx")]);
console.log(Object.keys(tree.children)); // the root nodes

// Any glTF viewer opens the result.
writeFileSync("model.glb", exportGlb([model, readFileSync("overlay.ifcx")]));
```

## Browser

With a bundler, import the package as usual; the bundler loads the wasm
module:

```js
import { compose, exportGlb, fetchImports } from "@openbim/ifcx";
```

Without one, serve the package's `web/` directory, map the name in an
import map (or import the file by URL), and call `init()` once before
anything else. `init()` fetches `openbim_ifcx_wasm_bg.wasm` from next to
the module; pass `init({ module_or_path: url })` to load it from elsewhere.

```html
<script type="importmap">
  { "imports": { "@openbim/ifcx/web": "/node_modules/@openbim/ifcx/web/openbim_ifcx_wasm.js" } }
</script>
<script type="module">
  import init, { IfcxFile, exportGlb, fetchImports, validate } from "@openbim/ifcx/web";

  await init();
  const url = new URL("models/house.ifcx", location.href);
  const model = new Uint8Array(await (await fetch(url)).arrayBuffer());

  // Fetch what the model imports, relative to its own URL, then use it.
  const imports = await fetchImports(model, { baseUrl: url });
  const report = validate(model, { imports });
  const glb = exportGlb(model, { imports }); // e.g. for three.js's GLTFLoader.parse
  console.log(IfcxFile.parse(model).header.id, report.valid, glb.length);
</script>
```

## API

| Call | Returns |
| --- | --- |
| `IfcxFile.parse(input)` | an `IfcxFile`; `input` is JSON text or UTF-8 bytes |
| `file.write(pretty?)` | the file as JSON text, lossless |
| `file.toJSON()` | the file as a plain object (`JSON.stringify(file)` works) |
| `file.header` | the `header` object |
| `file.nodeCount` | number of entries in `data` |
| `file.validate()` | a `ValidationReport` against the file's own `schemas` |
| `compose(layers, options?)` | the composed tree as a `ComposedNode` |
| `validate(layers, options?)` | a `ValidationReport` over all layers and resolved imports |
| `exportGlb(layers, options?)` | the composed model as binary glTF 2.0, a `Uint8Array` |
| `fetchImports(layers, options?)` | a promise of a `Map` from import `uri` to file, for `options.imports` |
| `init(input?)` | `@openbim/ifcx/web` only: loads the wasm module; await it once first |

`layers` is one input or an array of them, weakest first. TypeScript
declarations ship with the package.

**Lossless round trip.** `write()` keeps key order, fields the draft does
not define, `null` deletions and every number exactly as read. A plain
object from `toJSON()` cannot promise that: JavaScript lists integer-like
keys first and rounds integers beyond 2^53. Keep the `IfcxFile` and call
`write()` to save a file.

**Composed tree.** `compose` returns the artificial root (path `""`) over
every root node. Each node is `{ path, attributes, children }`, the shape of
upstream's `PostCompositionNode`, with `children` keyed by child name. Nodes
shared through inheritance appear once per place they are used.

**Imports.** Without `options.imports`, imports are not resolved: the layers'
schemas and data are concatenated in order, as upstream's `Federate` does.
With `options.imports` (an object or a `Map` from the exact import `uri` to
a file), the layers become the imports of a synthetic main layer, as
upstream's `ifcx compose` command builds it, and every import must be
supplied. `integrity` values are checked against the supplied bytes. In
upstream order an import overrides the layer that imports it, and a later
layer overrides both. The WebAssembly module never touches the network or
the filesystem.

**Fetching imports.** `fetchImports(layers, options?)` collects them for
you, in JavaScript: it reads each layer's `imports`, fetches every `uri`
(recursively, each once, concurrently) and resolves to a `Map` keyed by
the exact import `uri`, ready for `options.imports`.

- `baseUrl`: what relative URIs of the given layers resolve against;
  `location.href` by default in a browser. A fetched file's own imports
  resolve against its URL.
- `fetch`: the function that loads a URL, the global `fetch` by default.
  Pass your own for credentials, a cache, a mirror, or Node files:
  `{ fetch: async (url) => readFile(new URL(url)) }`. It may return a
  `Response`, a `Uint8Array`, an `ArrayBuffer` or a string. Without a
  `baseUrl` it receives a relative `uri` unchanged.
- `imports`: files already at hand, never fetched again.
- `signal`: an `AbortSignal` passed to every fetch.

The in-memory map keys a file by its exact `uri`, as upstream's
`InMemoryLayerProvider` does, so two different files imported under the
same relative `uri` from different directories cannot both be supplied;
the first one fetched is used. Anything that cannot be fetched rejects with
an `IfcxError` with code `fetch`; `integrity` and import cycles are checked
by `compose`, `validate` and `exportGlb`.

**GLB export.** `exportGlb` writes the render scene of the composed tree:
transformed, instanced meshes, lines and points with their materials, one
glTF node per instance named by its IFCX path. The root node turns IFCX's
Z-up into glTF's Y-up (`yUp: false` keeps Z-up) and carries the scene
origin, so georeferenced models keep their coordinates
(`originOnRoot: false` centres the model on the glTF origin instead).
Geometry values that do not decode are left out. `options.imports` works
as for `compose`.

**Validation report.** `{ valid, failures }`, each failure
`{ node, attribute, pointer, kind, message }`: the node path, the attribute
id, an RFC 6901 pointer into the value (`""` for the value itself), a stable
`kind` such as `missing-schema`, `type-mismatch`, `not-an-integer`,
`not-an-option`, `missing-key`, `too-few-elements` or `too-many-elements`,
and a human message.

## Errors

Every failure throws an `Error` with `name === "IfcxError"` and a stable
`code`:

| `code` | Meaning |
| --- | --- |
| `read` | not an IFCX file: invalid JSON or the wrong shape; the message gives line and column |
| `write` | the file could not be written |
| `layer` | imports could not be resolved: a missing file, an import cycle, an `integrity` mismatch |
| `compose` | a reference cycle, or a reference to a node no layer defines |
| `invalid-argument` | an argument of the wrong type, or no layers |
| `glb` | the scene could not be written as GLB (over 4 GiB) |
| `fetch` | `fetchImports` could not fetch an import, or got no file back |

A code is never renamed or reused.

## Building

```sh
cargo install wasm-bindgen-cli --version 0.2.128 --locked
crates/openbim-ifcx-wasm/scripts/build-npm-pkg.sh    # builds pkg/, runs every check
```

The script binds one release build three times (`pkg/`, `pkg/bundler/`,
`pkg/web/`), runs the Node suite, then `npm pack`s the package and checks
each entry point from the tarball: Node `require` and `import`, a webpack
bundle, and the `web` and bundler builds in headless Chrome, which parse,
validate, compose, fetch imports and export GLB from the repository's
fixtures. It finds Chrome through `CHROME_BIN`, `PATH` or a Playwright
download; `IFCX_SKIP_BROWSER=1` skips the browser part with a warning.

## License

MIT, like the rest of `openbimrs/ifcx`.
