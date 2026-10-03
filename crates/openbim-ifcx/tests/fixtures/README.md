# Test fixtures

Hand-written IFCX files for `openbim-ifcx` tests. They are original work for
this repository, licensed under MIT like the rest of it. No file is copied
from `buildingSMART/IFC5-development`, which publishes no license.

| File | Covers |
| --- | --- |
| `minimal.ifcx` | Smallest valid file: header and empty sections |
| `wall-with-type.ifcx` | Imports with and without `integrity`; every `dataType`; nested array and object restrictions; `children`, `inherits`, and attributes; two nodes sharing a path |
| `deletions.ifcx` | `null` entries in `children` and `inherits`; present but empty maps |
| `unknown-fields.ifcx` | Fields the draft does not define at every level, an unknown `dataType`, and numbers that only survive with exact float and integer handling |

Paths and URIs are illustrative; nothing resolves them.

`tests/upstream_round_trip.rs` checks upstream's own examples from a local
checkout named by `IFCX_UPSTREAM_DIR`, without committing them.
