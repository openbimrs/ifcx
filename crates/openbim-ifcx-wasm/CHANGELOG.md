# Changelog -- openbim-ifcx-wasm

All notable changes to the `openbim-ifcx-wasm` crate are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this crate follows Semantic Versioning independently of its siblings:
a release here does not imply a release of any other crate in this
repository.

## [Unreleased]

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

[Unreleased]: https://github.com/openbimrs/ifcx/compare/openbim-ifcx-wasm-v0.1.0...HEAD
[0.1.0]: https://github.com/openbimrs/ifcx/releases/tag/openbim-ifcx-wasm-v0.1.0
