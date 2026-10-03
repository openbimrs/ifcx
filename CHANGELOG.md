# Changelog

All notable changes to this project are documented in this file. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `openbim-ifcx-geometry` builds a flat render scene from a composition
  (`RenderScene::from_composition`, `from_root`): instances with node path,
  world transform, `f32` render matrix, material, and bounds; shared `f32`
  mesh (with vertex normals), line, and point buffers decoded once per shared
  attribute value and instanced; a material table; `f64` world bounds; and a
  render origin (`SceneOptions`, `Origin`) that keeps georeferenced models
  precise. Invisible subtrees are left out, materials resolve through
  ancestors, and malformed values become `SceneWarning`s. This completes
  world transforms through the hierarchy (#3) and presentation resolution
  (#7) (#8).
- `scripts/upstream-parity.sh`: opt-in check that composes every example of a
  local buildingSMART/IFC5-development checkout (`IFCX_UPSTREAM_DIR`), and
  the crate's fixtures, with upstream's TypeScript (bundled from the checkout
  at run time) and with the new `compose-json` example of `openbim-ifcx`, and
  reports matches and differences per case. All 47 upstream examples at
  `1a63082` compose equal to upstream (#17).
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
- `openbim-ifcx-geometry` decodes `ifcx_alpha` geometry attributes:
  `usd::xformop` into an affine `Transform` (USD row-vector layout, `f64`)
  with `world_from_parent` for hierarchy composition (#3, tree walk pending);
  `usd::usdgeom::mesh` into a `TriangleMesh` with face normals (#4); and
  `usd::usdgeom::basiscurves` into polylines, with non-linear curve types
  reported as unsupported (#5). Malformed values return a typed
  `DecodeError`.
- `openbim-ifcx-geometry` decodes point clouds per node from
  `points::array`, `points::base64`, and `pcd::base64` (PCD `ascii`,
  `binary`, `binary_compressed`) into `PointCloud` with `f64` positions and
  optional linear RGB colours. Malformed base64 or PCD returns
  `DecodeError::InvalidBase64`, `ByteLength`, or `InvalidPcd`; PCD layouts
  upstream cannot read either return `UnsupportedPcd` (#6).
- `openbim-ifcx-geometry` decodes visibility, `diffuseColor`, `opacity`, and
  `gltf::material` per node (`NodePresentation`) and resolves visibility and
  materials over a node's ancestors with the reference viewer's precedence
  (`is_visible`, `resolve_basic_material`, `resolve_mesh_material`) (#7).
- `openbim-ifcx-geometry` `Attributes` trait reads attributes from an
  `IfcxNode`, `FlatNode`, `ComposedNode`, or a plain map, treating `null` as
  absent.

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
