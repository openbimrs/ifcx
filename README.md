# OpenBIM.rs IFCX

Canonical OpenBIM.rs repository for **IFC5 / IFCX**, buildingSMART's layered,
componentised JSON model with USD-like composition.

IFCX is not an EXPRESS schema release. It therefore lives here as its own
standard family rather than as a schema version in
[`openbimrs/ifc`](https://github.com/openbimrs/ifc), which deliberately
rejects the `IFC5` schema token. See
[ADR 0001](docs/adr/0001-ifcx-is-its-own-family.md).

## Status

**Early.** Single files read and write losslessly, layers flatten and compose
into a resolved node tree, imports resolve through a caller-supplied
resolver, attributes are checked against the file's `schemas`, and a
composed tree turns into a flat render scene of transformed, instanced
meshes, lines, and point clouds with resolved materials, which exports as
GLB. The Rust crates are on crates.io, the JavaScript binding on npm and
the Python binding on PyPI, each released on its own through
`.github/workflows/release.yml`.
[`docs/capabilities.md`](docs/capabilities.md) is the
authoritative list of what exists.

**Documentation:** [openbimrs.github.io/ifcx](https://openbimrs.github.io/ifcx/):
install, getting started in Rust, JavaScript and Python, one reference page
per crate, the Rust API (rustdoc), capabilities, the upstream evidence, the
decision records and the changelog.

**Demo:** [openbimrs.github.io/ifcx](https://openbimrs.github.io/ifcx/)
composes an `.ifcx` file in your browser with `@openbim/ifcx`, exports GLB
and shows it with three.js, with the node tree, picked attributes and
validation failures ([`demo/`](demo/README.md)).

The crates, generated from their manifests and changelogs:

<!-- CRATES:TABLE:BEGIN -->

| Crate | Description | Distributed as | Latest release |
| --- | --- | --- | --- |
| [`openbim-ifcx`](https://openbimrs.github.io/ifcx/reference/crates/openbim-ifcx) | Lossless reader and writer for IFC5 / IFCX files, the layered JSON model. | [crates.io `openbim-ifcx`](https://crates.io/crates/openbim-ifcx) | 0.1.0 (2026-10-03) |
| [`openbim-ifcx-geometry`](https://openbimrs.github.io/ifcx/reference/crates/openbim-ifcx-geometry) | Renderer-neutral geometry and viewer helpers for IFC5 / IFCX: transforms, meshes, curves, point clouds, presentation, a flat render scene, and GLB export. | [crates.io `openbim-ifcx-geometry`](https://crates.io/crates/openbim-ifcx-geometry) | 0.1.0 (2026-10-03) |
| [`openbim-ifcx-binding-core`](https://openbimrs.github.io/ifcx/reference/crates/openbim-ifcx-binding-core) | Host-independent core shared by the openbim-ifcx language bindings (WebAssembly, Python). | not published on its own (`publish = false`); ships inside the bindings | not released |
| [`openbim-ifcx-py`](https://openbimrs.github.io/ifcx/reference/crates/openbim-ifcx-py) | Python bindings for openbim-ifcx: read, write, validate, compose and export IFC5 / IFCX files from Python. | [PyPI `openbim-ifcx`](https://pypi.org/project/openbim-ifcx/) | 0.1.1 (2026-10-03) |
| [`openbim-ifcx-wasm`](https://openbimrs.github.io/ifcx/reference/crates/openbim-ifcx-wasm) | WebAssembly bindings for openbim-ifcx: read, write, validate, compose and export IFC5 / IFCX files from JavaScript. | [npm `@openbim/ifcx`](https://www.npmjs.com/package/@openbim/ifcx) | 0.2.0 (2026-10-03) |

<!-- CRATES:TABLE:END -->

## Development

```bash
./scripts/gate.sh
cargo run -p xtask -- docs   # after changing a README, changelog, manifest or API
```

Rust 2021, MSRV 1.88, pure Rust, no `unsafe`. The `bindings` section of the
gate also needs `wasm-bindgen-cli` 0.2.128, Node, Chrome or Chromium, `uv`,
and `maturin`; see
[CONTRIBUTING.md](CONTRIBUTING.md), which also explains how the
documentation is generated and previewed.

## License

MIT. See [`LICENSE`](LICENSE) and [`LICENSING.md`](LICENSING.md).
