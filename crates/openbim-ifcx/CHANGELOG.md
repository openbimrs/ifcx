# Changelog -- openbim-ifcx

All notable changes to the `openbim-ifcx` crate are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this crate follows Semantic Versioning independently of its siblings:
a release here does not imply a release of any other crate in this
repository.

## [Unreleased]

## [0.1.0] - 2026-10-03

First release: lossless reading and writing, layer flattening and
composition, import resolution, and schema validation for `ifcx_alpha`.

### Added

- `openbim-ifcx` fixture `geometry-model.ifcx`: a small validated model with
  a column type mesh and axis shared by two placed occurrences, a slab, and
  USD transform, mesh, curve, and visibility attributes (#17).
- `openbim-ifcx` flattens layered nodes by path in layer order
  (`flatten`, `FlatNode`), matching upstream `FlattenCompositionInput`;
  attribute values are shared through `Arc` (#13).
- `openbim-ifcx` composes flattened nodes into a resolved tree (`compose`,
  `Composition`, `ComposedNode`, `ComposeError`), matching upstream
  `ComposeNode` and `CreateArtificialRoot`. Instances share composed
  sub-trees through `Arc`; reference cycles, including ones through
  `head/child` references that upstream misses, and unknown references are
  typed errors (#14).
- `openbim-ifcx::layers` resolves `imports` for `ifcx_alpha` through a
  caller-supplied `LayerResolver`: `LayerStackBuilder` loads the main layer
  and its imports once each in upstream order, `LayerStack::federate` merges
  schemas and data in that order, and typed `LayerError`s report missing
  layers, import cycles, unreadable layers, and `integrity` mismatches.
  `MemoryResolver` serves layers from memory; `FsResolver` (feature `fs`)
  reads local files and maps URI prefixes to offline mirrors. `integrity` is
  checked as SRI-style `sha256`/`sha384`/`sha512` digests (feature
  `integrity`, default on; without it such imports are rejected). In upstream
  order an import overrides the layer importing it; see
  `docs/capabilities.md` (#15).
- `openbim-ifcx` reads and writes `ifcx_alpha` files losslessly
  (`IfcxFile::from_json_*`, `to_json_*`): typed header, imports, schemas, and
  nodes; unknown fields kept at every level; `null` children and inherits
  preserved; read errors report kind, line, and column (#12).
- `openbim-ifcx` checks attribute values against the file's `schemas`
  (`IfcxFile::validate`, `validate_attributes`, `validate_nodes`), following
  upstream's `schema-validation.ts` but reporting every failure with node
  path, attribute id, and JSON pointer in a `ValidationReport`. `Integer`
  rejects fractions and array `min`/`max` are enforced, unlike upstream;
  `Blob` values are accepted unchecked and `quantityKind` is not checked
  (#16).

[Unreleased]: https://github.com/openbimrs/ifcx/compare/openbim-ifcx-v0.1.0...HEAD
[0.1.0]: https://github.com/openbimrs/ifcx/releases/tag/openbim-ifcx-v0.1.0
