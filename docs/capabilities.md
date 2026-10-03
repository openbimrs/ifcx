# Capabilities

Status vocabulary: **reserved** (named, no behavior), **implemented** (code
exists with tests), **conformance-tested** (checked against buildingSMART
samples for a named draft revision).

Target draft: `ifcx_alpha` (`schema/ifcx.tsp` in buildingSMART/IFC5-development).

The upstream evidence below was recorded at buildingSMART/IFC5-development
`1a63082`. It is re-checked against upstream's default branch every week by
`.github/workflows/upstream-drift.yml`, which runs every opt-in upstream
check and compares the results with `scripts/upstream-drift/baseline.txt`;
any failure or changed result opens an `upstream-drift` issue (see
[CONTRIBUTING.md](../CONTRIBUTING.md#upstream-drift)). A claim here stays
pinned to the revision it names until it is re-recorded.

| Capability | Status | IFCX draft | Crate | Notes |
| --- | --- | --- | --- | --- |
| Read and write an IFCX file losslessly | implemented | `ifcx_alpha` | `openbim-ifcx` | Content round-trips, including unknown fields and `null` deletions. Map keys keep their order; known fields are written in `ifcx.tsp` order. All 47 upstream example files (`1a63082`) round-trip via the opt-in `upstream_round_trip` test |
| Flatten layered nodes by path | implemented | `ifcx_alpha` | `openbim-ifcx` | `flatten`, upstream `FlattenCompositionInput` (`1a63082`): later nodes win for children and attributes, `null` inherits remove, `null` children are kept for composition. Keys keep insertion order; upstream's JavaScript objects list integer-like keys first. `flatten_owned` consumes the nodes and moves attribute values into the shared `Arc`s instead of copying them: in the opt-in `upstream_composition` test (release build) flattening `Tekla House` takes 18 ms instead of 88 ms and `Railway_project_IFC5` 8 ms instead of 60 ms |
| Compose layers into a resolved node tree | implemented | `ifcx_alpha` | `openbim-ifcx` | `compose`, upstream `ComposeNode`/`CreateArtificialRoot` (`1a63082`): `inherits` expansion, `head/a/b` references, `null` child deletion, local attributes over inherited, `head/a` paths editing children, artificial root over all roots. Sub-trees are shared through `Arc` and copied only along edited paths; no recursion per tree level. Cycles and unknown references are typed errors; cycles and roots use reference heads, as upstream's `TODO` asks (upstream misses cycles through `head/a` and overflows the stack), and a reference to a missing node is an error where upstream composes an empty node. The opt-in `upstream_composition` test composes all 47 upstream examples: 35 alone and 12 overlays on top of their example folder. `scripts/upstream-parity.sh` checks the composed trees equal to upstream's TypeScript for all 47 (`1a63082`); see [Upstream parity](#upstream-parity) |
| Check attributes against the file's `schemas` | implemented | `ifcx_alpha` | `openbim-ifcx` | Rules of upstream `schema-validation.ts` (`1a63082`), collecting every failure with node path, attribute id, and JSON pointer. Stricter than upstream for `Integer` fractions, array `min`/`max`, and non-object `Object` values; `Blob` is accepted unchecked, unknown `dataType`s are reported, `quantityKind` is not checked. Schema `inherits` is walked without recursion and each inherited schema is checked once per value, so long chains and diamond-shaped inheritance stay linear (a diamond's shared schema reports its failures once); inheritance cycles are reported. `IfcxFile::validate` checks only the file's own schemas; `LayerStack::validate` checks a stack built with its imports: the schemas of every layer, merged as `federate` does, against the attributes of every layer merged per path as flattening does; `validate_flat` checks flattened nodes. All 47 upstream examples validate via the opt-in `upstream_validation` test when their imported schema files are supplied, and the 37 upstream stacks that resolve against an offline `ifcx.dev` mirror validate with `LayerStack::validate` in the opt-in `upstream_layers` test (`1a63082`) |
| Resolve imports | implemented | `ifcx_alpha` | `openbim-ifcx` | `layers::LayerStackBuilder` over a caller-supplied `LayerResolver`; each layer loads once, in upstream `IfcxLayerStackBuilder` order (`1a63082`); `federate` merges schemas and data in that order. Typed errors for missing layers, cycles, and `integrity` mismatch (SRI-style `sha256`/`sha384`/`sha512`, feature `integrity`, default on). `FsResolver` behind feature `fs`; no network access. `LayerStackBuilder::build_all` stacks several layers as the imports of a main layer without data, as upstream's `ifcx compose` does. All 41 upstream examples with imports build stacks via the opt-in `upstream_layers` test against an offline `ifcx.dev` mirror, except 4 that import `ifc-mat/prop@v1.0.0.ifcx`, which `ifcx.dev` does not serve |
| World transforms through the node hierarchy | implemented | `ifcx_alpha` | `openbim-ifcx-geometry` | `usd::xformop` `{transform}` decodes to an affine `f64` `Transform` in USD's row-vector layout (translation in the last row, as upstream's viewer reads it); non-affine or malformed matrices are typed errors. `world_from_parent` composes a child's world matrix, inheriting when absent. The render scene walks the composed tree with it and exposes each drawn node's path and `f64` world matrix (`Instance::world`); the root's own transform applies, where upstream's viewer skips it on its attribute-less artificial root. All 41 840 upstream transforms (`1a63082`) decode via the opt-in `upstream_decode` test |
| Triangle meshes | implemented | `ifcx_alpha` | `openbim-ifcx-geometry` | `usd::usdgeom::mesh` `{points, faceVertexIndices}` decodes to `TriangleMesh` (`f64` positions, `u32` indices, flat face normals). Out-of-range indices, counts not a multiple of 3, non-triangle `faceVertexCounts`, and wrong shapes are typed errors; polygons are not triangulated. All 1 925 upstream meshes (720 241 triangles) decode |
| Polylines | implemented | `ifcx_alpha` | `openbim-ifcx-geometry` | `usd::usdgeom::basiscurves` `{points, curveVertexCounts?}` decodes to one `Polyline` per curve. A missing `type` is linear, as in upstream's viewer (USD's default would be cubic); any other `type` returns `CurveGeometry::Unsupported`. `wrap: periodic` closes the line. All 85 upstream curve attributes decode as linear |
| Point clouds | implemented | `ifcx_alpha` | `openbim-ifcx-geometry` | Per node: `points::array` (JSON triples), `points::base64` (little-endian `f32` triples, 12-byte stride), `pcd::base64` (PCD v0.7 `ascii`, `binary`, `binary_compressed`; fields `x y z` as 4-byte floats and optional packed `rgb`, other layouts a typed `UnsupportedPcd`). `f64` positions (JSON kept exact, upstream narrows to `f32`), linear RGB colours (8-bit PCD `rgb` converted from sRGB as three.js does), upstream viewer precedence `pcd` > `array` > `base64`. All 8 point-cloud attributes in the upstream examples (`1a63082`, 102,761 points) decode via the opt-in `upstream_decode` test |
| Visibility, colour, opacity, materials | implemented | `ifcx_alpha` | `openbim-ifcx-geometry` | Per node: `usd::usdgeom::visibility`, `bsi::ifc::presentation::diffuseColor` and `opacity`, `gltf::material` (PBR factors, alpha, double-sided; texture locations recorded, not loaded). Precedence over a node's ancestors as pure functions, tested on a composed tree with a material bound through `inherits`; the render scene drops invisible subtrees and resolves each instance's material through its ancestors into a deduplicated material table. `usd::usdshade::*` bindings are not read: `ifcx_alpha` binds materials through `inherits`, and no upstream example uses them. All upstream presentation attributes (`1a63082`) decode via `upstream_decode` |
| Flat render scene for viewers | implemented | `ifcx_alpha` | `openbim-ifcx-geometry` | `RenderScene::from_composition` walks the composed tree once, without recursion, following upstream `render.ts` (`1a63082`): instances with node path, defining node, `f64` world transform, `f32` render matrix, material, and world bounds; mesh (area-weighted vertex normals as three.js), line-segment, and point buffers in `f32`, each decoded once per shared attribute value (`Arc` identity from composition) and instanced; a material table; `f64` world bounds. Buffers are relative to their own bounding-box centre and matrices to a render origin (default: centre of the world bounds), so georeferenced vertices keep sub-millimetre precision. Malformed values become `SceneWarning`s with path and attribute instead of failing the scene; a malformed transform drops its subtree. The walk is bounded by `SceneOptions::max_visits` (10 million node visits by default; shared sub-trees can describe exponentially many paths) and `max_path_bytes` (1 GiB of instance paths), stopping with a `LimitReached` warning; geometry and instances whose coordinates do not fit the `f32` buffers and matrices are left out with an `OutOfRange` warning, so every value in a scene is finite. Differences from the viewer: all geometry kinds of one node are drawn (none of the upstream examples has two), lines keep the resolved colour (the viewer darkens by 0.8), colourless `pcd::base64` points are black (the viewer's are white). All 47 upstream examples build scenes with no warnings via the opt-in `upstream_scene` test: 26 218 instances, 4 181 777 triangles, 9 884 segments, 102 761 points, 128 ms in total (release build) |
| GLB export | implemented | `ifcx_alpha` | `openbim-ifcx-geometry` | `to_glb` writes a `RenderScene` as one binary glTF 2.0 file with `serde_json`, no glTF library: `TRIANGLES` with `POSITION`/`NORMAL`/`u32` indices, `LINES`, `POINTS` with `COLOR_0`; one glTF mesh per (buffer, material) pair, shared by every instance node; node names are IFCX paths (`extras.ifcxNode` the defining node); PBR factors, `alphaMode`, `alphaCutoff`, `doubleSided` (textures not written); exact `f32` `POSITION` min/max; 4-byte alignment; every JSON number finite (no `null`), checked by the `scene_glb` fuzz target. A root node carries the scene origin and the Z-up to Y-up rotation (both optional; the origin is always in `extras.ifcxOrigin`). The `ifcx2glb` example converts files; with `--resolve-imports` (and `--mirror PREFIX=DIR` for offline copies) it loads every layer's imports through `FsResolver` first. Against an offline `ifcx.dev` mirror it exported 34 of the 47 upstream examples alone (`1a63082`), byte-identical to an export without imports, since their imports carry no `data`; the other 13 are the 4 that import the unserved `prop@v1.0.0.ifcx` and 9 overlays that need their example folder underneath. A hand-written layer whose panels inherit a type only its import defines exports only with imports resolved (`tests/imports.rs`). The opt-in `scripts/gltf-validate.sh` ran the Khronos glTF validator 2.0.0-dev.3.10 on the 4 fixtures (`imports-panel-type` with its import resolved) and all 47 upstream examples (`1a63082`): 0 errors, 0 warnings; 16 infos, all `ACCESSOR_INDEX_TRIANGLE_DEGENERATE` from degenerate triangles in two upstream files' meshes |
| IFC4 to IFCX bridge | reserved | — | `openbim-ifcx-ifc4` (not created) | Would depend on `openbim-ifc`; never the reverse |
| JavaScript and Python bindings | implemented | `ifcx_alpha` | `openbim-ifcx-wasm` (npm `@openbim/ifcx`), `openbim-ifcx-py` (PyPI `openbim-ifcx`) | Read a file from text or bytes and write it back losslessly; validate it or a set of layers into a `{valid, failures}` report with stable `kind` codes; compose layers (weakest first; imports resolved from memory with `LayerStackBuilder::build_all`, as upstream's `ifcx compose`), validate them with their imports' schemas into a `{path, attributes, children}` tree; export the composed model as GLB (`exportGlb`, `export_glb`) through `openbim-ifcx-geometry`. Thin hosts over `openbim-ifcx-binding-core`; every failure has a stable error `code`. npm: Node CommonJS, bundler and `web` ES module builds, the browser builds tested in headless Chrome; imports fetched in JavaScript by `fetchImports`, never by the wasm module; abi3 wheels for CPython 3.9+ |

## Fuzzing

Every capability above that reads untrusted input has a cargo-fuzz target in
`fuzz/` (see CONTRIBUTING.md, "Fuzzing"): the reader (with a lossless
round-trip check), flatten and compose, validation, the PCD reader and its
LZF decoder, base64 and array point buffers, the mesh, curve, transform and
presentation decoders, and the render scene with GLB export. CI runs each
for one minute per relevant pull request and ten minutes weekly. Fixes for
findings carry regression tests in the crates' own suites (#41).

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

## Upstream parity

Upstream publishes no license, so its examples are not vendored and parity
with its TypeScript reference implementation is the conformance evidence.
`scripts/upstream-parity.sh` (opt-in, not in `gate.sh`) bundles
`LoadIfcxFile` from a local checkout named by `IFCX_UPSTREAM_DIR` with a
pinned esbuild, composes each case with it and with this crate's
`compose-json` example, and compares the trees as JSON values. Imports are
not resolved: every `ifcx_alpha` example imports only schema files, which
carry no `data`, so they cannot change a composed tree.

Last recorded: buildingSMART/IFC5-development `1a63082`, draft `ifcx_alpha`,
59 cases. Re-run weekly against upstream's default branch by the upstream
drift workflow.

| Cases | Result |
| --- | --- |
| 35 upstream examples composed alone | equal to upstream |
| 12 upstream examples that reference nodes of other files (`Geotech` 2, `Hello Wall/advanced` 3, `Tunnel Excavation` 7), composed on top of every other file of their example folder in sorted order | equal to upstream |
| 10 hand-written fixtures composed alone (`tests/fixtures/*.ifcx`) | 8 equal; `layer-base` and `layer-edit` reference nodes they do not define |
| 2 hand-written layer stacks (`layer-base` + `layer-edit`, `layers/chain`) | reference nodes they do not define |

- Upstream writes `-0` as `0` (JavaScript `JSON.stringify`); this crate keeps
  `-0.0` as read. The comparison reads `-0` as `0`; 11 examples and
  `geometry-model.ifcx` contain it (`Tekla House` 3 480 times).
- Key order of attributes and children was also identical in every equal
  case.
- Known divergence: a reference to a node that no layer defines is
  `ComposeError::UnknownReference` here, while upstream composes an empty
  node. The 4 fixture cases above report this and are not counted as
  differences.

## Not here

- IFC2X3, IFC4, and IFC4X3 in STEP or XML: [`openbimrs/ifc`](https://github.com/openbimrs/ifc).
- buildingSMART's draft ifcJSON encoding of the IFC4 data model: not planned
  anywhere in OpenBIM.rs (openbimrs/ifc#122 was closed).
