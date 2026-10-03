# Changelog

Repository-level changes: tooling, CI, documentation, and decisions. Each
crate owns a `CHANGELOG.md` beside its `Cargo.toml` and is versioned
independently; add crate changes there:

- [`openbim-ifcx`](crates/openbim-ifcx/CHANGELOG.md)
- [`openbim-ifcx-geometry`](crates/openbim-ifcx-geometry/CHANGELOG.md)
- [`openbim-ifcx-binding-core`](crates/openbim-ifcx-binding-core/CHANGELOG.md)
- [`openbim-ifcx-wasm`](crates/openbim-ifcx-wasm/CHANGELOG.md) (npm `@openbim/ifcx`)
- [`openbim-ifcx-py`](crates/openbim-ifcx-py/CHANGELOG.md) (PyPI `openbim-ifcx`)

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

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

### Changed

- Workspace crates live under `crates/`, as in `openbimrs/ifc`.
- License changed from AGPL-3.0 to MIT before any code or release.
- MSRV is 1.88, matching `openbim-ifc` and `openbim-step`.
