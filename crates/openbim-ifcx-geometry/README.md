# openbim-ifcx-geometry

Renderer-neutral geometry and viewer helpers for IFC5 / IFCX. It depends on
`openbim-ifcx` and never on a renderer or GPU API.

Implemented for the `ifcx_alpha` draft:

| Module | Attribute | Result |
| --- | --- | --- |
| `transform` | `usd::xformop` | Affine `Transform` (USD row-vector layout, translation in the last row) and `world_from_parent` |
| `mesh` | `usd::usdgeom::mesh` | `TriangleMesh`: positions, `u32` indices, flat face normals |
| `curves` | `usd::usdgeom::basiscurves` | Linear `Polyline`s, or `Unsupported` for other curve types |
| `points` | `points::array`, `points::base64`, `pcd::base64` | `PointCloud`: positions and optional linear RGB colours; PCD `ascii`, `binary`, `binary_compressed` |
| `presentation` | `usd::usdgeom::visibility`, `bsi::ifc::presentation::*`, `gltf::material` | `NodePresentation` per node; `is_visible`, `resolve_basic_material`, `resolve_mesh_material` apply the reference viewer's precedence over a node's ancestors |

Each decoder takes an attribute's `serde_json::Value` and returns a typed
`DecodeError` for malformed input. Coordinates are `f64` because IFCX files
carry georeferenced coordinates; convert to `f32` for rendering after
applying world transforms.

Planned: the render scene over a composed node tree and GLB export. See the
[repository capabilities](https://github.com/openbimrs/ifcx/blob/main/docs/capabilities.md).

Licensed under MIT.
