# Changelog -- openbim-ifcx-geometry

All notable changes to the `openbim-ifcx-geometry` crate are documented here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this crate follows Semantic Versioning independently of its siblings:
a release here does not imply a release of any other crate in this
repository.

## [Unreleased]

## [0.1.1] - 2026-10-03

No breaking change. Requires `openbim-ifcx` 0.1.1.

### Added

- `ifcx2glb --resolve-imports` loads every layer's `imports` through
  `openbim_ifcx::layers::FsResolver` before exporting, stacked as upstream's
  `ifcx compose` does; `--mirror PREFIX=DIR` (repeatable) maps a URI prefix
  such as `https://ifcx.dev/` to an offline copy. The example now flattens
  with `flatten_owned` (#42, #43).
- Test fixture `imports-panel-type.ifcx`, whose panels inherit a type only
  its import defines, and `tests/imports.rs`, which exports it with the
  import resolved (#42).
- Dev-dependency on `openbim-ifcx` with feature `fs`, for the example and
  its test.
- `SceneOptions::max_visits` (default 10 million) and
  `SceneOptions::max_path_bytes` (default 1 GiB), with builders and
  `DEFAULT_*` constants, bound the render-scene walk. When one is reached
  the walk stops with a `SceneWarningKind::LimitReached` warning (#41).
- `SceneWarningKind::OutOfRange` for geometry and instances whose
  coordinates the scene's `f32` buffers and matrices cannot hold (#41).

### Fixed

- `RenderScene::from_composition` no longer visits an exponential number of
  paths: composition shares sub-trees, so 40 nodes naming the next one twice
  as children described 2^40 instances. Deep trees with long child names no
  longer build instance paths without bound. Found while fuzzing (#41).
- Coordinates beyond `f32` range (for example a translation of `5e299`)
  no longer produce infinite render matrices, buffers, or origins, which
  `to_glb` wrote as JSON `null`; such geometry and instances are left out
  with an `OutOfRange` warning. `to_glb` writes an infinite `alphaCutoff` as
  `f32::MAX` and a NaN one as `0.5`. Found by fuzzing (#41).
- `PointCloud::from_pcd` returns `InvalidPcd` for ASCII data whose `COUNT`
  values overflow `usize` when summed, instead of panicking with overflow
  checks on (or reading the wrong column without). Found while fuzzing
  (#41).

## [0.1.0] - 2026-10-03

First release: decoders for `ifcx_alpha` transforms, meshes, curves, point
clouds and presentation; a flat render scene over a composed tree; and GLB
export.

### Added

- `openbim-ifcx-geometry` writes a `RenderScene` as binary glTF 2.0
  (`to_glb`, `GlbOptions`, `GlbError`): shared meshes per buffer and
  material, `TRIANGLES`, `LINES`, and `POINTS` with `COLOR_0`, PBR
  materials, node names set to IFCX paths, and a root node with the scene
  origin and the Z-up to Y-up rotation. The `ifcx2glb` example converts IFCX
  layers to `.glb`; the opt-in `scripts/gltf-validate.sh` checks exports with
  the Khronos glTF validator (#9).
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
- `openbim-ifcx-geometry` scaffold for renderer-neutral geometry, viewer
  helpers, and GLB export.

### Removed

- `openbim_ifcx_geometry::PACKAGE_STATUS`, replaced by the decoders.
