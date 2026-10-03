# Test fixtures

Hand-written IFCX files for `openbim-ifcx-geometry` tests. They are original
work for this repository, licensed under MIT like the rest of it. No file is
copied from `buildingSMART/IFC5-development`, which publishes no license.

| File | Covers |
| --- | --- |
| `placed-storey.ifcx` | Three-level hierarchy: translated site, rotated and lifted storey, a wall mesh without its own transform, and a translated linear axis curve |
| `instanced-types.ifcx` | A panel type with a glTF material bound through `inherits`, instanced three times (one mirrored) and once hidden with a child; a wall with a bound colour over a mesh child and a closed two-curve axis; malformed colour, mesh, and cubic curve on one node; point clouds with and without colours |
| `imports-panel-type.ifcx` | A row of two placed panels whose type, mesh, and material come only from its import `instanced-types.ifcx`, for import resolution before export |
| `georeferenced.ifcx` | A terrain mesh at UTM-sized coordinates with millimetre offsets, and a curve placed by a large translation |

`tests/upstream_decode.rs` decodes, and `tests/upstream_scene.rs` builds
render scenes of, upstream's own examples from a local checkout named by
`IFCX_UPSTREAM_DIR`, without committing them.
