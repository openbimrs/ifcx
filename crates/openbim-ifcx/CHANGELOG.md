# Changelog -- openbim-ifcx

All notable changes to the `openbim-ifcx` crate are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this crate follows Semantic Versioning independently of its siblings:
a release here does not imply a release of any other crate in this
repository.

## [Unreleased]

### Changed

- **Breaking:** a layer now overrides the layers it imports, as agreed
  upstream in buildingSMART/IFC5-development#144 (2026-10-05): "imported
  data should come 'before' the main data". `LayerStackBuilder` returns the
  layers in federation order: every layer after its own imports,
  recursively; sibling imports in written order, so a later import
  overrides an earlier one; each layer once, where it is first reached; the
  main layer last. `main → [a, b]`, `a → [c]` now gives `c, a, b, main`
  (was `main, a, c, b`), and `main → a → b` gives `b, a, main` (was
  `main, a, b`). Code relying on the old order, in which an import
  overrode its importer and the main layer had the lowest priority, now
  composes and validates differently whenever an import carries `data` or
  redefines a schema. Upstream's code at `1a63082` still uses the old order
  until a pending upstream pull request lands (#36).
- **Breaking:** `LayerStack::layers`, `keys` and `into_layers` return the
  federation order, so the main layer is last, not first;
  `LayerStack::main` still returns the main layer. For `build_all`, `main`
  is now the last layer of the stack, the strongest named one (was the
  first named one), and the federated header is its header (#36).
- **Breaking:** `federate` and `federate_owned` take the header of the last
  file, the strongest, instead of the first; schemas and data are merged as
  before. `LayerStack::federate` and `into_federated` thus keep the main
  layer's header (#36).
- `LayerStackBuilder::allow_cycles(true)` skips the import that closes a
  cycle, as before; under the new order the layer that import names comes
  after its importer, and the main layer stays last (`main → a → main`
  gives `a, main`) (#36).
- `LayerStackBuilder` walks imports without recursion, so a long import
  chain cannot overflow the stack. `integrity` is still checked on every
  import edge (#36).

## [0.1.1] - 2026-10-03

No breaking change: every addition below is new API, and existing
functions and types keep their signatures.

### Added

- `LayerStack::validate` checks a layer stack built with its imports: the
  schemas of every layer, merged as `federate` does, against the attributes
  of every layer, merged per path as `flatten` does. An attribute whose
  schema lives only in an imported file now validates. `validate_flat`
  checks the output of `flatten` or `flatten_owned` (#42).
- `LayerStackBuilder::build_all` stacks several layers, weakest first, as
  the imports of a main layer without data, as upstream's `ifcx compose`
  does (#42).
- `flatten_owned`, `FlatNode::merge_owned`, `federate_owned`, and
  `LayerStack::into_federated` consume their input and move nodes and
  attribute values instead of copying them. In the opt-in
  `upstream_composition` test (release build) flattening `Tekla House`
  drops from 88 ms to 18 ms and `Railway_project_IFC5` from 60 ms to 8 ms
  (#43).

### Changed

- `IfcxFile::validate` merges attributes per path by reference instead of
  copying every value; the report is unchanged (#43).

### Migration

- To flatten without copying, replace `flatten(&file.data)` with
  `flatten_owned(file.data)` where the nodes are not needed afterwards, and
  `stack.federate()` with `stack.into_federated()`. `flatten` over borrowed
  nodes still copies each attribute value, as before.
- To validate a file with its imports, build its layer stack and call
  `stack.validate()` instead of `file.validate()`.

### Fixed

- `IfcxFile::validate` and `validate_attributes` walk schema `inherits`
  with an explicit stack and check each inherited schema once per value.
  A long inheritance chain overflowed the stack, and diamond-shaped
  inheritance doubled the work per level (22 levels took seconds and
  reported one failure four million times); a schema reached along several
  paths now reports its failures once. Found while fuzzing (#41).

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

[Unreleased]: https://github.com/openbimrs/ifcx/compare/openbim-ifcx-v0.1.1...HEAD
[0.1.1]: https://github.com/openbimrs/ifcx/releases/tag/openbim-ifcx-v0.1.1
[0.1.0]: https://github.com/openbimrs/ifcx/releases/tag/openbim-ifcx-v0.1.0
