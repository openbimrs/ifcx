# Changelog -- openbim-ifcx-wasm

All notable changes to the `openbim-ifcx-wasm` crate are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this crate follows Semantic Versioning independently of its siblings:
a release here does not imply a release of any other crate in this
repository.

## [Unreleased]

## [0.2.1] - 2026-10-03

### Changed

- Built on `openbim-ifcx` 0.1.1 and `openbim-ifcx-geometry` 0.1.1:
  validation with imports checks every layer against the schemas of all
  layers, flattening and composition no longer copy attribute values, and
  the fuzzing fixes apply (deep schema inheritance, oversized coordinates,
  exponential scene walks, malformed PCD headers).

- The README, which is the npm page, links the documentation site
  (<https://openbimrs.github.io/ifcx/>) and the viewer at its new address,
  <https://openbimrs.github.io/ifcx/viewer/> (#64).

## [0.2.0] - 2026-10-03

### Added

- Browser builds in the same npm package (#46): next to the Node CommonJS
  build, `bundler/` (`wasm-bindgen --target bundler`, the default export
  for bundlers such as webpack) and `web/` (`--target web`, imported as
  `@openbim/ifcx/web`, with an async `init()` that loads the wasm module).
  `package.json` `exports` selects the build (`node` condition: CommonJS;
  otherwise the bundler build), with TypeScript declarations for each.
  The Node API and entry point are unchanged.
- `fetchImports(layers, { baseUrl, fetch, imports, signal })`: resolves
  the layers' imports recursively in JavaScript with `fetch` (or any
  function returning a `Response`, bytes or text) and returns them keyed
  by import `uri` for the existing in-memory `imports` option. The Rust
  crates still perform no network access (ADR 0002). A file that cannot be
  fetched rejects with an `IfcxError` with the new code `fetch`.
- `scripts/build-npm-pkg.sh` (was `build-node-pkg.sh`) binds all three
  targets, runs the Node suite, and `tools/check-package.mjs` checks the
  packed tarball: Node `require` and `import`, a webpack bundle (webpack
  pinned in `tools/package-lock.json`), and both browser builds in
  headless Chrome, parsing, validating, composing, fetching imports and
  exporting GLB from the repository's fixtures.

### Changed

- Through `openbim-ifcx-binding-core`: layers with `imports` are stacked by
  `openbim-ifcx`'s `LayerStackBuilder::build_all`, and composition moves
  the parsed layers instead of copying them. Layer order, results and
  reports are unchanged (#42, #43).

## [0.1.0] - 2026-10-03

First version, for npm as `@openbim/ifcx` (`publish = false` on crates.io).

### Added

- WebAssembly bindings for `openbim-ifcx`, a CommonJS package for Node 18+
  with TypeScript declarations. `IfcxFile.parse` reads a file from a string
  or a `Uint8Array`; `write` writes it back losslessly; `toJSON`, `header`,
  `nodeCount` and `validate` expose it as plain objects. `compose(layers,
  { imports })` returns the composed tree and `validate(layers, { imports })`
  a structured report over all layers, with imports resolved from memory
  when given. `exportGlb(layers, { imports, yUp, originOnRoot })` writes the
  composed model as binary glTF 2.0.
- Every failure throws an `IfcxError` with a stable `code`.
- `scripts/build-node-pkg.sh` builds the package with the pinned
  `wasm-bindgen` CLI and runs the Node suite against it.

[Unreleased]: https://github.com/openbimrs/ifcx/compare/openbim-ifcx-wasm-v0.2.1...HEAD
[0.2.1]: https://github.com/openbimrs/ifcx/releases/tag/openbim-ifcx-wasm-v0.2.1
[0.2.0]: https://github.com/openbimrs/ifcx/releases/tag/openbim-ifcx-wasm-v0.2.0
[0.1.0]: https://github.com/openbimrs/ifcx/releases/tag/openbim-ifcx-wasm-v0.1.0
