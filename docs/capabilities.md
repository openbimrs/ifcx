# Capabilities

Status vocabulary: **reserved** (named, no behavior), **implemented** (code
exists with tests), **conformance-tested** (checked against buildingSMART
samples for a named draft revision).

Target draft: `ifcx_alpha` (`schema/ifcx.tsp` in buildingSMART/IFC5-development).

| Capability | Status | IFCX draft | Crate | Notes |
| --- | --- | --- | --- | --- |
| Read and write an IFCX file losslessly | implemented | `ifcx_alpha` | `openbim-ifcx` | Content round-trips, including unknown fields and `null` deletions. Map keys keep their order; known fields are written in `ifcx.tsp` order. All 47 upstream example files (`1a63082`) round-trip via the opt-in `upstream_round_trip` test |
| Flatten layered nodes by path | implemented | `ifcx_alpha` | `openbim-ifcx` | `flatten`, upstream `FlattenCompositionInput` (`1a63082`): later nodes win for children and attributes, `null` inherits remove, `null` children are kept for composition. Keys keep insertion order; upstream's JavaScript objects list integer-like keys first |
| Compose layers into a resolved node tree | implemented | `ifcx_alpha` | `openbim-ifcx` | `compose`, upstream `ComposeNode`/`CreateArtificialRoot` (`1a63082`): `inherits` expansion, `head/a/b` references, `null` child deletion, local attributes over inherited, `head/a` paths editing children, artificial root over all roots. Sub-trees are shared through `Arc` and copied only along edited paths; no recursion per tree level. Cycles and unknown references are typed errors; cycles and roots use reference heads, as upstream's `TODO` asks (upstream misses cycles through `head/a` and overflows the stack), and a reference to a missing node is an error where upstream composes an empty node. The opt-in `upstream_composition` test composes all 47 upstream examples: 35 alone and 12 overlays on top of their example folder; output was checked equal to upstream's TypeScript for all 47 |
| Check attributes against the file's `schemas` | implemented | `ifcx_alpha` | `openbim-ifcx` | Rules of upstream `schema-validation.ts` (`1a63082`), collecting every failure with node path, attribute id, and JSON pointer. Stricter than upstream for `Integer` fractions, array `min`/`max`, and non-object `Object` values; `Blob` is accepted unchecked, unknown `dataType`s are reported, `quantityKind` is not checked. All 47 upstream examples validate via the opt-in `upstream_validation` test when their imported schema files are supplied |
| Resolve imports | implemented | `ifcx_alpha` | `openbim-ifcx` | `layers::LayerStackBuilder` over a caller-supplied `LayerResolver`; each layer loads once, in upstream `IfcxLayerStackBuilder` order (`1a63082`); `federate` merges schemas and data in that order. Typed errors for missing layers, cycles, and `integrity` mismatch (SRI-style `sha256`/`sha384`/`sha512`, feature `integrity`, default on). `FsResolver` behind feature `fs`; no network access. All 41 upstream examples with imports build stacks via the opt-in `upstream_layers` test against an offline `ifcx.dev` mirror, except 4 that import `ifc-mat/prop@v1.0.0.ifcx`, which `ifcx.dev` does not serve |
| World transforms through the node hierarchy | implemented | `ifcx_alpha` | `openbim-ifcx-geometry` | `usd::xformop` `{transform}` decodes to an affine `f64` `Transform` in USD's row-vector layout (translation in the last row, as upstream's viewer reads it); non-affine or malformed matrices are typed errors. `world_from_parent` composes a child's world matrix, inheriting when absent. Walking the composed node tree waits for composition and the render scene (#8). All 41 840 upstream transforms (`1a63082`) decode via the opt-in `upstream_decode` test |
| Triangle meshes | implemented | `ifcx_alpha` | `openbim-ifcx-geometry` | `usd::usdgeom::mesh` `{points, faceVertexIndices}` decodes to `TriangleMesh` (`f64` positions, `u32` indices, flat face normals). Out-of-range indices, counts not a multiple of 3, non-triangle `faceVertexCounts`, and wrong shapes are typed errors; polygons are not triangulated. All 1 925 upstream meshes (720 241 triangles) decode |
| Polylines | implemented | `ifcx_alpha` | `openbim-ifcx-geometry` | `usd::usdgeom::basiscurves` `{points, curveVertexCounts?}` decodes to one `Polyline` per curve. A missing `type` is linear, as in upstream's viewer (USD's default would be cubic); any other `type` returns `CurveGeometry::Unsupported`. `wrap: periodic` closes the line. All 85 upstream curve attributes decode as linear |
| Point clouds | reserved | — | `openbim-ifcx-geometry` | `points::array::*`, `points::base64::*`, `pcd::base64` |
| Visibility, colour, opacity, materials | reserved | — | `openbim-ifcx-geometry` | `usd::usdgeom::visibility`, `bsi::ifc::presentation::*`, `usd::usdshade::*`, `gltf::material::*` |
| Flat render scene for viewers | reserved | — | `openbim-ifcx-geometry` | |
| GLB export | reserved | — | `openbim-ifcx-geometry` | |
| IFC4 to IFCX bridge | reserved | — | `openbim-ifcx-ifc4` (not created) | Would depend on `openbim-ifc`; never the reverse |

## Layer order and import priority

Checked against `src/ifcx-core/layers/layer-stack.ts` and `Federate` in
`src/ifcx-core/workflows.ts` of buildingSMART/IFC5-development at `1a63082`.

- The main layer is first. Each layer claims all of its not yet loaded
  imports in written order, then each claimed import is followed by its own
  new imports before the next one: `main → [a, b]`, `a → [c]` gives
  `main, a, c, b`.
- `Federate` concatenates schemas and data in that order, and composition
  lets later opinions win. **So an import overrides the layer that imports
  it**, later imports override earlier ones, and the main layer has the
  lowest priority. A layer cannot override what it imports; to override a
  dataset, a viewer or CLI lists it before the overriding file in a
  synthetic main layer's imports (upstream's `compose3` and `ifcx compose`
  do this with the files in user order, so the last file wins).
- The `ifcx_alpha` draft text (`schema/ifcx.tsp`, README, `Examples_FAQ.md`)
  states no priority between a layer and its imports; the FAQ only says an
  added layer "will provide the new value". ADR 0002's "later opinions
  override earlier ones" holds within this order. This is the opposite of
  USD sublayers, where the importing layer is stronger.
- Divergences from upstream: upstream's builder appends a nested subtree more
  than once (`main → a → c` yields `main, c, a, c`); later occurrences win, so
  composed values match the deduplicated order this crate uses, but the
  position where a path or schema id first appears can differ. Upstream
  accepts import cycles silently; this crate rejects them unless
  `allow_cycles(true)` is set. Upstream does not check `integrity` (TODO in
  its providers) and defines no format for it.

## Not here

- IFC2X3, IFC4, and IFC4X3 in STEP or XML: [`openbimrs/ifc`](https://github.com/openbimrs/ifc).
- buildingSMART's draft ifcJSON encoding of the IFC4 data model: not planned
  anywhere in OpenBIM.rs (openbimrs/ifc#122 was closed).
