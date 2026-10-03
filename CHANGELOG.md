# Changelog

All notable changes to this project are documented in this file. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

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
- `openbim-ifcx-geometry` decodes `ifcx_alpha` geometry attributes:
  `usd::xformop` into an affine `Transform` (USD row-vector layout, `f64`)
  with `world_from_parent` for hierarchy composition (#3, tree walk pending);
  `usd::usdgeom::mesh` into a `TriangleMesh` with face normals (#4); and
  `usd::usdgeom::basiscurves` into polylines, with non-linear curve types
  reported as unsupported (#5). Malformed values return a typed
  `DecodeError`.

- Repository scaffold: `openbim-ifcx` crate with a status constant only,
  verification gate, pinned CI, and Dependabot for action pins.
- ADR 0001 recording that IFC5 / IFCX is its own family, not an IFC schema
  version.
- ADR 0002 fixing the crate split: one core crate for file model and
  composition, a separate crate for the IFC4 bridge.
- `openbim-ifcx-geometry` scaffold for renderer-neutral geometry, viewer
  helpers, and GLB export.
- Issue templates, labels, Discussions, and the IFCX project board.
- Capability table stating that no IFCX behavior is implemented yet.

### Changed

- Workspace crates live under `crates/`, as in `openbimrs/ifc`.

- License changed from AGPL-3.0 to MIT before any code or release.
- MSRV is 1.88, matching `openbim-ifc` and `openbim-step`.

### Removed

- `openbim_ifcx_geometry::PACKAGE_STATUS`, replaced by the decoders.
