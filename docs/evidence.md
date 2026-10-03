# Upstream parity and drift

IFCX is an alpha draft developed in
[buildingSMART/IFC5-development](https://github.com/buildingSMART/IFC5-development).
That repository publishes no license, so none of its files, examples or
texts are copied here or published on this site. Conformance is shown
instead by running this repository's code against a local checkout of
upstream and recording the results: counts and failures, never content.

## What is checked

- **Round trip, validation, composition, layers, decoding, scenes:** opt-in
  Rust tests (`upstream_*`) read every upstream example from the checkout
  named by `IFCX_UPSTREAM_DIR`.
- **Parity:** `scripts/upstream-parity.sh` bundles upstream's own
  TypeScript composition from the checkout at run time and compares its
  composed trees with this crate's, case by case, as JSON values. The
  cases and the known divergences are listed under
  [Capabilities: upstream parity](/capabilities#upstream-parity).
- **glTF:** `scripts/gltf-validate.sh` exports every example to GLB and
  runs the Khronos glTF validator on it.
- **Imports:** the `ifcx.dev` files the examples import are fetched into
  an offline mirror for the run, and their digests recorded.

## Recorded results

These are the summary lines of the last accepted run, from
[`scripts/upstream-drift/baseline.txt`](https://github.com/openbimrs/ifcx/blob/main/scripts/upstream-drift/baseline.txt).
Every number in [Capabilities](/capabilities) is pinned to the same
revision.

<!-- UPSTREAM:BASELINE:BEGIN -->

Recorded against buildingSMART/IFC5-development [`1a63082`](https://github.com/buildingSMART/IFC5-development/commit/1a63082ada967c683cfacee2005f8f749c8e1b79).

| Check | What it runs | Recorded result |
| --- | --- | --- |
| `imports` | Fetches every `ifcx.dev` file the examples import into an offline mirror (`scripts/upstream-drift/fetch-imports.py`) | `7 URIs, 1 unavailable` |
|  |  | `import https://ifcx.dev/@nlsfb/nlsfb@v1.ifcx sha256:22c3f74fa121296e` |
|  |  | `import https://ifcx.dev/@openusd.org/usd@v1.ifcx sha256:ec9d50cd00e20c8b` |
|  |  | `import https://ifcx.dev/@standards.buildingsmart.org/ifc/core/ifc@v5a.ifcx sha256:7ada4aea92eefd97` |
|  |  | `import https://ifcx.dev/@standards.buildingsmart.org/ifc/core/prop@v5a.ifcx sha256:5bd444faf1b515b2` |
|  |  | `import https://ifcx.dev/@standards.buildingsmart.org/ifc/ifc-infra/infra@v1.0.0.ifcx sha256:b1905b68ab20b943` |
|  |  | `import https://ifcx.dev/@standards.buildingsmart.org/ifc/ifc-mat/ifc-mat@v1.0.0.ifcx sha256:0bfecb9f32ce8f10` |
|  |  | `import https://ifcx.dev/@standards.buildingsmart.org/ifc/ifc-mat/prop@v1.0.0.ifcx unavailable (HTTP 404)` |
| `round-trip` | Reads and writes every example, comparing content (`upstream_round_trip`) | `47 files, 0 failures` |
| `validation` | Validates every example against its own and its imported schemas (`upstream_validation`) | `47 files: 47 valid, 0 need imported schemas, 0 invalid` |
| `composition` | Composes every example, alone or on top of its example folder (`upstream_composition`) | `47 files: 35 compose alone, 12 on their folder, 0 fail` |
| `layers` | Builds and validates every import stack against the mirror (`upstream_layers`) | `37 stacks built (37 valid), 4 unresolved, 0 failures` |
|  |  | `unresolved   4x https://ifcx.dev/@standards.buildingsmart.org/ifc/ifc-mat/prop@v1.0.0.ifcx` |
| `decode` | Decodes every transform, mesh, curve, point cloud and presentation attribute (`upstream_decode`) | `47 files: 41840 transforms; 1925 meshes (720241 triangles, 522435 vertices); 85 curve attributes (85 polylines, 9946 vertices, 0 unsupported); 0 null deletions; 0 failures` |
|  |  | `47 files: point clouds pcd 2, array 4, base64 2 (102761 points, 4 coloured); visibility 62, diffuseColor 9366, opacity 458, gltf::material 2 (2 PBR); 0 failures` |
| `scene` | Builds the render scene of every example (`upstream_scene`) | `47 files: 26218 instances, 4181777 triangles, 9884 segments, 102761 points, 0 warnings` |
| `parity` | Compares composed trees with upstream's TypeScript (`scripts/upstream-parity.sh`) | `59 cases: 55 match, 4 known divergences, 0 differ` |
| `gltf` | Exports fixtures and examples to GLB and runs the Khronos glTF validator (`scripts/gltf-validate.sh`) | `51 files: 0 errors, 0 warnings, 16 infos, 0 hints` |

<!-- UPSTREAM:BASELINE:END -->

## Drift

The draft keeps moving. Every Monday, and on demand,
[`.github/workflows/upstream-drift.yml`](https://github.com/openbimrs/ifcx/blob/main/.github/workflows/upstream-drift.yml)
runs `scripts/upstream-drift.sh` against upstream's default branch and
compares each summary line with the baseline above. A failed check or a
changed line opens, or updates, the single open issue labelled
[`upstream-drift`](https://github.com/openbimrs/ifcx/issues?q=label%3Aupstream-drift),
naming the upstream commit and linking the logs. The workflow is not part
of CI and never runs on a push or pull request.

When the evidence changes on purpose, `docs/capabilities.md` and the
baseline are updated together (`./scripts/upstream-drift.sh
--update-baseline`), naming the new revision, and this page follows on the
next `cargo run -p xtask -- docs`. [Contributing](/project/contributing#upstream-drift)
describes how to triage a drift issue and reproduce it locally.
