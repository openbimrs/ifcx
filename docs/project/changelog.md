# Changelog

Assembled from every changelog in the repository: the root
[`CHANGELOG.md`](https://github.com/openbimrs/ifcx/blob/main/CHANGELOG.md)
for tooling, CI and documentation, and each crate's own `CHANGELOG.md`.
Every crate is versioned and released on its own, so a release is named by
crate and version. Each crate's [reference page](/reference/) shows its
latest release notes.

<!-- CHANGELOG:BEGIN -->

## Unreleased

### Repository (tooling, CI, documentation)

#### Added

- Documentation site (VitePress) for <https://openbimrs.github.io/ifcx/>:
  home, install and getting-started guides for Rust, JavaScript and
  Python whose code runs in the gate, one reference page per crate,
  capabilities, upstream parity and drift evidence, ADRs, the assembled
  changelog, contributing and releasing. `xtask` (`publish = false`)
  generates every derivable page and region from the manifests, READMEs,
  changelogs, sources and binding declarations;
  `cargo run -p xtask -- docs --check`, `todo --check`, the site build
  and `scripts/check-leakage.py` run in the gate's `rust` section (#64).
- The demo viewer works on phones: a compact top bar, a 3D view that fills
  the screen, nodes and details in a bottom sheet behind tabs, touch
  orbit and tap-to-pick, and framing that fits portrait screens (#62).
- `demo/`: a static three.js viewer (Vite, pinned versions and lockfile)
  that composes `.ifcx` files in the browser with `@openbim/ifcx` built
  from source, exports GLB, and shows the node tree, picked meshes' IFCX
  paths and attributes, and validation failures. Samples are the
  repository's own fixtures. `.github/workflows/pages.yml` deploys it to
  <https://openbimrs.github.io/ifcx/> (#47).

- The `bindings` gate section and CI also check `@openbim/ifcx` as packed,
  including its browser builds in headless Chrome
  (`crates/openbim-ifcx-wasm/scripts/build-npm-pkg.sh`, #46).
- Weekly upstream drift check (`.github/workflows/upstream-drift.yml`,
  `scripts/upstream-drift.sh`): runs every opt-in upstream check against
  buildingSMART/IFC5-development's default branch with an `ifcx.dev` import
  mirror fetched in the job, compares the results with
  `scripts/upstream-drift/baseline.txt` (`1a63082`), reports which upstream
  branches moved since the last run, and opens or updates a single
  `upstream-drift` issue on failure or difference. Not part of the CI gate
  (#35).
- Fuzzing (#41): `fuzz/` holds cargo-fuzz targets `read`, `compose`,
  `validate`, `pcd`, `pcd_lzf`, `points_base64`, `geometry`, and
  `scene_glb`, in their own workspace with a pinned nightly, outside the MSRV
  gate. `fuzz/run.sh` runs one with seeds from `fuzz/seeds/` and the
  hand-written fixtures, debug assertions, and memory and time limits.
  `.github/workflows/fuzz.yml` runs every target for one minute on pull
  requests touching the fuzzed crates and for ten minutes weekly, keeping
  the corpus; it is not part of the required gate. CONTRIBUTING.md documents
  it.

- Language bindings: `openbim-ifcx-wasm` (npm `@openbim/ifcx`) and
  `openbim-ifcx-py` (PyPI `openbim-ifcx`) over a shared
  `openbim-ifcx-binding-core`, as in `openbimrs/ifc`.
- Release workflow publishes `openbim-ifcx-wasm` to npm and
  `openbim-ifcx-py` to PyPI (four abi3 wheels and an sdist) by trusted
  publishing in the `npmjs.com` and `pypi.org` environments;
  `scripts/release-crate.py` keeps `package.json` and `pyproject.toml` in
  step with `Cargo.toml`. A `workflow_dispatch` run rehearses a release.
- `scripts/gate.sh` has sections `rust` and `bindings`; the bindings section
  builds the npm package and the wheel and runs the Node and Python suites
  against them. CI runs the sections as parallel jobs with a single
  `Standalone IFCX gate` verdict.
- `scripts/gltf-validate.sh`: opt-in export of the geometry fixtures and,
  with `IFCX_UPSTREAM_DIR`, every upstream example to GLB, validated with the
  pinned Khronos glTF validator (`scripts/gltf/`). All 50 files at `1a63082`
  pass with no errors or warnings (#9).
- Release tooling: `scripts/release-crate.py` and `.github/workflows/release.yml`
  publish one crate per `<crate>-v<version>` tag to crates.io, as in
  `openbimrs/ifc`. Per-crate changelogs replace the single root entry list.
- `scripts/upstream-parity.sh`: opt-in check that composes every example of a
  local buildingSMART/IFC5-development checkout (`IFCX_UPSTREAM_DIR`), and
  the crate's fixtures, with upstream's TypeScript (bundled from the checkout
  at run time) and with the new `compose-json` example of `openbim-ifcx`, and
  reports matches and differences per case. All 47 upstream examples at
  `1a63082` compose equal to upstream (#17).
- Repository scaffold: `openbim-ifcx` crate with a status constant only,
  verification gate, pinned CI, and Dependabot for action pins.
- ADR 0001 recording that IFC5 / IFCX is its own family, not an IFC schema
  version.
- ADR 0002 fixing the crate split: one core crate for file model and
  composition, a separate crate for the IFC4 bridge.
- Issue templates, labels, Discussions, and the IFCX project board.

#### Changed

- Workspace crates live under `crates/`, as in `openbimrs/ifc`.
- License changed from AGPL-3.0 to MIT before any code or release.
- MSRV is 1.88, matching `openbim-ifc` and `openbim-step`.

#### Fixed

- `scripts/gltf-validate.sh` exports the fixtures with `--resolve-imports`,
  so `imports-panel-type.ifcx` converts, and names an upstream example it
  cannot export instead of exiting silently. Found by the first upstream
  drift run (#56).

### `openbim-ifcx`

No breaking change: every addition below is new API, and existing
functions and types keep their signatures.

#### Added

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

#### Changed

- `IfcxFile::validate` merges attributes per path by reference instead of
  copying every value; the report is unchanged (#43).

#### Migration

- To flatten without copying, replace `flatten(&file.data)` with
  `flatten_owned(file.data)` where the nodes are not needed afterwards, and
  `stack.federate()` with `stack.into_federated()`. `flatten` over borrowed
  nodes still copies each attribute value, as before.
- To validate a file with its imports, build its layer stack and call
  `stack.validate()` instead of `file.validate()`.

#### Fixed

- `IfcxFile::validate` and `validate_attributes` walk schema `inherits`
  with an explicit stack and check each inherited schema once per value.
  A long inheritance chain overflowed the stack, and diamond-shaped
  inheritance doubled the work per level (22 levels took seconds and
  reported one failure four million times); a schema reached along several
  paths now reports its failures once. Found while fuzzing (#41).

### `openbim-ifcx-geometry`

#### Added

- `ifcx2glb --resolve-imports` loads every layer's `imports` through
  `openbim_ifcx::layers::FsResolver` before exporting, stacked as upstream's
  `ifcx compose` does; `--mirror PREFIX=DIR` (repeatable) maps a URI prefix
  such as `https://ifcx.dev/` to an offline copy. The example now flattens
  with `flatten_owned` (#42, #43).
- Test fixture `imports-panel-type.ifcx`, whose panels inherit a type only
  its import defines, and `tests/imports.rs`, which exports it with the
  import resolved (#42).
- Dev-dependency on `openbim-ifcx` with feature `fs`, for the example and
  its test. The example and tests use API added to `openbim-ifcx` after
  0.1.0, so the `openbim-ifcx` requirement must move to that release when
  this crate is next released.
- `SceneOptions::max_visits` (default 10 million) and
  `SceneOptions::max_path_bytes` (default 1 GiB), with builders and
  `DEFAULT_*` constants, bound the render-scene walk. When one is reached
  the walk stops with a `SceneWarningKind::LimitReached` warning (#41).
- `SceneWarningKind::OutOfRange` for geometry and instances whose
  coordinates the scene's `f32` buffers and matrices cannot hold (#41).

#### Fixed

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

### `openbim-ifcx-binding-core`

Not released on its own (`publish = false`): it ships inside the npm and
PyPI packages, whose changelogs name the releases.

#### Changed

- `LayerSet` with imports stacks its layers with
  `LayerStackBuilder::build_all` instead of building its own synthetic main
  layer; the order and the reports are unchanged. Federation and
  composition move the parsed layers instead of copying them
  (`federate_owned`, `LayerStack::into_federated`, `flatten_owned`) (#42,
  #43).

#### Added

- The host-independent half of the language bindings: `Document` (read,
  lossless write, header, validation against the file's own schemas),
  `LayerSet` (layers weakest first, optional in-memory imports under a
  synthetic main layer, composition into a `{path, attributes, children}`
  tree, federated validation, GLB export through `openbim-ifcx-geometry`), validation reports with stable failure
  `kind` codes, and `BindingError` with the stable codes `read`, `write`,
  `layer`, `compose`, `invalid-argument` and `glb`.

## Releases

### `openbim-ifcx-wasm` 0.2.0 (2026-10-03)

#### Added

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

#### Changed

- Through `openbim-ifcx-binding-core`: layers with `imports` are stacked by
  `openbim-ifcx`'s `LayerStackBuilder::build_all`, and composition moves
  the parsed layers instead of copying them. Layer order, results and
  reports are unchanged (#42, #43).

### `openbim-ifcx-py` 0.1.1 (2026-10-03)

#### Fixed

- The source distribution carries `LICENSE` at its root, where its metadata
  names it. PyPI rejected the 0.1.0 sdist for the missing file, so 0.1.0 is
  available as wheels only.

### `openbim-ifcx-geometry` 0.1.0 (2026-10-03)

First release: decoders for `ifcx_alpha` transforms, meshes, curves, point
clouds and presentation; a flat render scene over a composed tree; and GLB
export.

#### Added

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

#### Removed

- `openbim_ifcx_geometry::PACKAGE_STATUS`, replaced by the decoders.

### `openbim-ifcx-py` 0.1.0 (2026-10-03)

First version, for PyPI as `openbim-ifcx` (`publish = false` on crates.io).

#### Added

- Python bindings for `openbim-ifcx`, built with pyo3 and maturin as one
  abi3 wheel for CPython 3.9+. `IfcxFile.parse` reads a file from `str` or
  `bytes`; `write` writes it back losslessly; `to_dict`, `header`,
  `node_count` and `validate` expose it as plain dicts. `compose(layers,
  imports=None)` returns the composed tree and `validate(layers,
  imports=None)` a structured report over all layers, with imports resolved
  from memory when given. `export_glb(layers, imports=None, *, y_up=True,
  origin_on_root=True)` writes the composed model as binary glTF 2.0.
  Parsing, composing, validating and GLB export release the GIL.
- Every failure raises `IfcxError` with a stable `code`.
- `scripts/check-python.sh` builds the wheel, installs it into a throwaway
  uv venv and runs the Python suite against it.

### `openbim-ifcx-wasm` 0.1.0 (2026-10-03)

First version, for npm as `@openbim/ifcx` (`publish = false` on crates.io).

#### Added

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

### `openbim-ifcx` 0.1.0 (2026-10-03)

First release: lossless reading and writing, layer flattening and
composition, import resolution, and schema validation for `ifcx_alpha`.

#### Added

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

<!-- CHANGELOG:END -->
