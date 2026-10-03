# Test fixtures

Hand-written IFCX files for `openbim-ifcx` tests. They are original work for
this repository, licensed under MIT like the rest of it. No file is copied
from `buildingSMART/IFC5-development`, which publishes no license.

| File | Covers |
| --- | --- |
| `minimal.ifcx` | Smallest valid file: header and empty sections |
| `wall-with-type.ifcx` | Imports with and without `integrity`; every `dataType`; nested array and object restrictions; `children`, `inherits`, and attributes; two nodes sharing a path |
| `deletions.ifcx` | `null` entries in `children` and `inherits`; present but empty maps |
| `valid-attributes.ifcx` | Attributes of every checked `dataType` that match their schemas; nested arrays, optional object keys, schema `inherits`, an `__internal` attribute, and a later opinion replacing an earlier value |
| `invalid-attributes.ifcx` | Several failing attributes across two paths: wrong types, a fraction for `Integer`, an unknown enum option, a missing schema, array bounds, a missing object key, and a key needing JSON pointer escaping |
| `layer-base.ifcx`, `layer-edit.ifcx` | Two layers editing the same nodes: attribute override, `null` attribute, child replacement, `null` child, `null` inherit, a path addressing a child (`wall/Body`), and two nodes sharing a path within one layer |
| `occurrence-type.ifcx` | Two wall occurrences inheriting a type with body and axis children; a local attribute overriding an inherited one; a `null` child deleting an inherited one; paths editing an inherited child two levels deep and a missing child; a `head/child` reference; two roots |
| `occurrence-type.composed.json` | The tree upstream's TypeScript `CreateArtificialRoot` composes from `occurrence-type.ifcx`, as `{path, attributes, children}` objects. Generated from the fixture above; it holds no upstream content |
| `unknown-fields.ifcx` | Fields the draft does not define at every level, an unknown `dataType`, and numbers that only survive with exact float and integer handling |

| `layers/chain/main.ifcx`, `mid.ifcx`, `sub/base.ifcx` | Three-file import chain `main → mid → sub/base` with a relative path into a subdirectory and a correct `sha256` integrity value; each layer sets the same attribute, adds a schema, and `mid` deletes a child that `base` defines |
| `layers/cycle/a.ifcx`, `b.ifcx` | Two layers importing each other |
| `layers/missing/main.ifcx` | Import of a file that does not exist |
| `layers/integrity-mismatch/main.ifcx` | Import of `chain/sub/base.ifcx` with a wrong `sha256` integrity value |

Paths and URIs in the top-level files are illustrative; nothing resolves
them. The `layers/` files resolve relative to each other. Integrity values
hash the committed bytes, so `.gitattributes` keeps `*.ifcx` line endings
unchanged.

`tests/upstream_round_trip.rs`, `tests/upstream_validation.rs`, and
`tests/upstream_composition.rs` check
upstream's own examples from a local checkout named by `IFCX_UPSTREAM_DIR`,
without committing them.
`tests/upstream_layers.rs` (feature `fs`) builds layer stacks for those
examples; `IFCX_IMPORTS_MIRROR` names an offline copy of the `ifcx.dev` files
they import, laid out as `<host>/<path>`.
