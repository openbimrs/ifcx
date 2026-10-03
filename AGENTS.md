# IFCX repository instructions

This repository owns the OpenBIM.rs implementation of IFC5 / IFCX. The
lossless file model, layer composition (flattening and the composed tree),
attribute validation, import resolution (`layers`), the transform, mesh,
curve, point-cloud, and presentation attribute decoders, the flat render
scene, and GLB export are implemented. Do not describe a capability as
implemented without executable evidence here.

## Map

- `crates/openbim-ifcx/` — file model, JSON read/write, composition (`src/compose/`), attribute validation (`src/validate.rs`), layer stacks (`src/layers/`, features `integrity` and `fs`)
- `crates/openbim-ifcx-geometry/` — per-node attribute decoders (transforms, meshes, curves, point clouds in `points.rs`, presentation in `presentation.rs`) the flat render scene (`scene.rs`), and GLB export (`glb.rs`, example `ifcx2glb`); depends on the core crate, never on a renderer
- `crates/openbim-ifcx-binding-core/` — host-independent core of the bindings (`Document`, `LayerSet`, `BindingError` codes); JSON text to the hosts; `publish = false`
- `crates/openbim-ifcx-wasm/` — JavaScript binding, npm `@openbim/ifcx` (`npm/package.json`, `scripts/build-npm-pkg.sh`, `js/fetch-imports.js`, `tests/js/`, `tools/check-package.mjs`); `publish = false` on crates.io
- `crates/openbim-ifcx-py/` — Python binding, PyPI `openbim-ifcx` (`pyproject.toml`, `python/openbim_ifcx/`, `scripts/check-python.sh`, `tests/python/`); `publish = false` on crates.io
- `docs/capabilities.md` — authoritative capability table
- `docs/adr/` — architecture decisions; 0002 fixes the crate split
- `scripts/release-crate.py`, `.github/workflows/release.yml` — per-crate releases to crates.io, npm (`openbim-ifcx-wasm`), and PyPI (`openbim-ifcx-py`) by trusted publishing; see CONTRIBUTING.md
- `scripts/gate.sh` — complete local/CI verification gate; sections `rust` and `bindings` (CI runs them as parallel jobs)
- `scripts/upstream-parity.sh`, `scripts/parity/` — opt-in composition parity check against upstream's TypeScript; never vendors upstream code
- `scripts/upstream-drift.sh`, `scripts/upstream-drift/`, `.github/workflows/upstream-drift.yml` — every opt-in upstream check in one run, compared with `scripts/upstream-drift/baseline.txt`; the workflow runs it weekly and keeps one `upstream-drift` issue (CONTRIBUTING.md#upstream-drift)
- `CHANGELOG.md` — repository-level changes; each crate keeps its own `CHANGELOG.md`

## Commands

```bash
./scripts/gate.sh
./scripts/gate.sh rust          # one section; `bindings` needs wasm-bindgen-cli 0.2.128, node, uv, maturin
cargo test --workspace
crates/openbim-ifcx-wasm/scripts/build-npm-pkg.sh    # npm package (Node, bundler, web) + Node suite + headless-browser check
crates/openbim-ifcx-py/scripts/check-python.sh       # wheel + Python suite
# opt-in, needs a local buildingSMART/IFC5-development checkout
IFCX_UPSTREAM_DIR=../IFC5-development cargo test --release -p openbim-ifcx --test upstream_round_trip
# also needs the imported schema files from ifcx.dev, named by URI's last segment
IFCX_UPSTREAM_DIR=../IFC5-development IFCX_IMPORTS_DIR=../ifcx-imports \
    cargo test --release -p openbim-ifcx --test upstream_validation
IFCX_UPSTREAM_DIR=../IFC5-development cargo test --release -p openbim-ifcx --test upstream_composition -- --nocapture
IFCX_UPSTREAM_DIR=../IFC5-development cargo test --release -p openbim-ifcx-geometry --test upstream_decode
IFCX_UPSTREAM_DIR=../IFC5-development cargo test --release -p openbim-ifcx-geometry --test upstream_scene -- --nocapture
# Khronos glTF validator over exported GLBs (installs gltf-validator into scripts/gltf/node_modules)
IFCX_UPSTREAM_DIR=../IFC5-development ./scripts/gltf-validate.sh
# convert for a glTF viewer
cargo run --release -p openbim-ifcx-geometry --example ifcx2glb -- in.ifcx out.glb
# resolving imports from disk, ifcx.dev from an offline mirror
cargo run --release -p openbim-ifcx-geometry --example ifcx2glb -- \
    --resolve-imports --mirror https://ifcx.dev/=../ifcx-mirror/ifcx.dev in.ifcx out.glb
# also needs an offline mirror of the ifcx.dev files, laid out as <host>/<path>
IFCX_UPSTREAM_DIR=../IFC5-development IFCX_IMPORTS_MIRROR=../ifcx-mirror \
    cargo test --release -p openbim-ifcx --features fs --test upstream_layers -- --nocapture
# composed trees vs upstream's TypeScript; needs Node.js >= 22 and npm (installs pinned esbuild)
IFCX_UPSTREAM_DIR=../IFC5-development ./scripts/upstream-parity.sh
# all of the above in one run (fetches the ifcx.dev imports itself), compared with
# scripts/upstream-drift/baseline.txt; --update-baseline rewrites it
IFCX_UPSTREAM_DIR=../IFC5-development ./scripts/upstream-drift.sh
```

Trust command exit codes. Never summarize a Cargo pipeline in a way that hides
the Cargo process status.

## Boundaries

- IFCX may depend on released `openbim-step`, `openbim-core`, and `openbim-ifc`
  crates, for example for an IFC4 to IFCX bridge.
- `openbimrs/ifc` must never depend on IFCX. If both need something, move it
  down into `core` or `step`.
- IFCX is not a `SchemaVersion` of `openbim-ifc`. Do not add one there.
- Do not vendor buildingSMART schemas or sample files without verified
  redistribution rights. Record the IFCX draft revision a capability targets.
- Release-critical package metadata and cross-repository dependency versions
  are explicit in crate manifests.
- Bindings stay thin: IFCX behaviour goes into `openbim-ifcx` (or the
  geometry crate) first, shared host glue into `openbim-ifcx-binding-core`;
  the wasm and py crates only convert arguments and results. Error codes and
  validation `kind` codes may be added, never renamed.
- The npm and PyPI versions must equal the crate version;
  `scripts/release-crate.py --set --apply` bumps both. Never publish by
  hand except the first npm version (see CONTRIBUTING.md).

## Upstream drift

`.github/workflows/upstream-drift.yml` runs `scripts/upstream-drift.sh`
against upstream's default branch every Monday and on dispatch. It is not
part of CI and never runs on push or pull request. A failed check or a
changed summary line opens, or updates, the single open issue labelled
`upstream-drift`. When the evidence changes on purpose, refresh
`docs/capabilities.md` and `scripts/upstream-drift/baseline.txt` together
(`--update-baseline`), naming the new upstream revision.

## Documentation discipline

IFCX is still a moving draft. Keep the capability table honest and name the
draft revision for each claim. Update README, rustdoc, capabilities, and
CHANGELOG together for user-visible changes.
