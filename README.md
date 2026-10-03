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
GLB. No crate is published.
[`docs/capabilities.md`](docs/capabilities.md) is the
authoritative list of what exists.

| Crate | Role | Published |
| --- | --- | --- |
| `openbim-ifcx` | IFCX file model, lossless JSON read/write, flattening and composition, import resolution, attribute validation | crates.io from 0.1.0 |
| `openbim-ifcx-geometry` | Transform, mesh, curve, point-cloud, and presentation decoders; flat render scene; GLB export | no |

## Development

```bash
./scripts/gate.sh
```

Rust 2021, MSRV 1.88, pure Rust, no `unsafe`.

## License

MIT. See [`LICENSE`](LICENSE) and [`LICENSING.md`](LICENSING.md).
