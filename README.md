# OpenBIM.rs IFCX

Canonical OpenBIM.rs repository for **IFC5 / IFCX**, buildingSMART's layered,
componentised JSON model with USD-like composition.

IFCX is not an EXPRESS schema release. It therefore lives here as its own
standard family rather than as a schema version in
[`openbimrs/ifc`](https://github.com/openbimrs/ifc), which deliberately
rejects the `IFC5` schema token. See
[ADR 0001](docs/adr/0001-ifcx-is-its-own-family.md).

## Status

**Scaffold.** No reader, writer, or layer composition is implemented, and
no crate is published. [`docs/capabilities.md`](docs/capabilities.md) is the
authoritative list of what exists.

| Crate | Role | Published |
| --- | --- | --- |
| `openbim-ifcx` | IFCX model and composition (planned) | no |
| `openbim-ifcx-geometry` | Geometry and viewer helpers, GLB export (planned) | no |

## Development

```bash
./scripts/gate.sh
```

Rust 2021, MSRV 1.88, pure Rust, no `unsafe`.

## License

MIT. See [`LICENSE`](LICENSE) and [`LICENSING.md`](LICENSING.md).
