# @openbim/ifcx

Read, write, validate and compose **IFC5 / IFCX** files, and export them as
GLB, from JavaScript and TypeScript. A WebAssembly build of the
[`openbim-ifcx`](https://crates.io/crates/openbim-ifcx) Rust crate, from the
`openbim-ifcx-wasm` crate in [openbimrs/ifcx](https://github.com/openbimrs/ifcx).

Published to npm as `@openbim/ifcx`, a CommonJS build for Node 18 and later.
Browser bundling works in principle (the crate builds for
`wasm32-unknown-unknown`) but has no tested recipe yet.

Targets the `ifcx_alpha` draft of buildingSMART's IFC5. IFCX is still a
moving draft; see the
[capabilities](https://github.com/openbimrs/ifcx/blob/main/docs/capabilities.md).

## Install

```sh
npm install @openbim/ifcx
```

## Example

```js
const { readFileSync } = require("node:fs");
const { writeFileSync } = require("node:fs");
const { IfcxFile, compose, exportGlb } = require("@openbim/ifcx");

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
layer overrides both. Nothing is fetched from the network or the
filesystem.

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

A code is never renamed or reused.

## Building

```sh
cargo install wasm-bindgen-cli --version 0.2.128 --locked
crates/openbim-ifcx-wasm/scripts/build-node-pkg.sh   # builds pkg/ and runs the Node suite
```

## License

MIT, like the rest of `openbimrs/ifcx`.
