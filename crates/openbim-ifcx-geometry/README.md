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
| `glb` | a `RenderScene` | `to_glb`: one binary glTF 2.0 file with shared meshes, `LINES`, `POINTS` (with `COLOR_0`), PBR materials, node names set to IFCX paths, and the origin and Z-up to Y-up rotation on a root node |
| `scene` | all of the above, over a composed tree | `RenderScene::from_composition`: instances (node path for picking, `f64` world transform, `f32` render matrix, material), shared `f32` mesh, line, and point buffers stored once per shared attribute value, a material table, `f64` world bounds, a render origin, and per-value warnings |

Each decoder takes an attribute's `serde_json::Value` and returns a typed
`DecodeError` for malformed input. Coordinates are `f64` because IFCX files
carry georeferenced coordinates; convert to `f32` for rendering after
applying world transforms.

```rust
use openbim_ifcx::{compose, flatten_owned, IfcxFile};
use openbim_ifcx_geometry::{RenderScene, SceneOptions};

let file = IfcxFile::from_json_slice(&std::fs::read("model.ifcx")?)?;
let scene = RenderScene::from_composition(&compose(&flatten_owned(file.data))?, &SceneOptions::default());
for instance in &scene.instances {
    // instance.path, instance.geometry, instance.matrix, scene.materials[instance.material]
}
```

Buffers hold vertices relative to a per-buffer anchor, and instance matrices
map to world coordinates minus `scene.origin`, so georeferenced models keep
sub-millimetre precision in `f32`.

To look at an IFCX file in any glTF viewer:

```sh
cargo run --release -p openbim-ifcx-geometry --example ifcx2glb -- model.ifcx model.glb
# layers, weakest first; --z-up keeps IFCX axes, --local leaves the origin off the root
cargo run --release -p openbim-ifcx-geometry --example ifcx2glb -- base.ifcx overlay.ifcx out.glb
# resolve imports from disk; --mirror maps a URI prefix to an offline copy
cargo run --release -p openbim-ifcx-geometry --example ifcx2glb -- \
    --resolve-imports --mirror https://ifcx.dev/=mirror/ifcx.dev model.ifcx model.glb
```

Without `--resolve-imports`, `imports` are ignored. With it, every layer's
imports load through `openbim_ifcx::layers::FsResolver`, in upstream order
(each layer's imports override it, the next layer overrides both); nothing
is fetched from the network.

`scripts/gltf-validate.sh` (opt-in, needs Node.js) exports the fixtures and,
with `IFCX_UPSTREAM_DIR`, every upstream example, and checks them with the
Khronos glTF validator. See the
[repository capabilities](https://github.com/openbimrs/ifcx/blob/main/docs/capabilities.md).

Licensed under MIT.
