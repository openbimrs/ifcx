# Test fixtures

Hand-written IFCX files for `openbim-ifcx-geometry` tests. They are original
work for this repository, licensed under MIT like the rest of it. No file is
copied from `buildingSMART/IFC5-development`, which publishes no license.

| File | Covers |
| --- | --- |
| `placed-storey.ifcx` | Three-level hierarchy: translated site, rotated and lifted storey, a wall mesh without its own transform, and a translated linear axis curve |

`tests/upstream_decode.rs` decodes upstream's own examples from a local
checkout named by `IFCX_UPSTREAM_DIR`, without committing them.
