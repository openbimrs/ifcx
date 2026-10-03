//! Renderer-neutral geometry and viewer helpers for IFC5 / IFCX.
//!
//! The crate decodes the geometry and presentation attributes of
//! `ifcx_alpha` nodes into typed values. It never depends on a renderer or
//! GPU API (see `docs/adr/0002`).
//!
//! | Module | Attribute | Result |
//! | --- | --- | --- |
//! | [`transform`] | `usd::xformop` | [`Transform`], composed with [`world_from_parent`] |
//! | [`mesh`] | `usd::usdgeom::mesh` | [`TriangleMesh`] |
//! | [`curves`] | `usd::usdgeom::basiscurves` | [`CurveGeometry`] |
//! | [`points`] | `points::array`, `points::base64`, `pcd::base64` | [`PointCloud`] |
//! | [`presentation`] | `usd::usdgeom::visibility`, `bsi::ifc::presentation::*`, `gltf::material` | [`NodePresentation`], resolved over ancestors by [`is_visible`], [`resolve_basic_material`], and [`resolve_mesh_material`] |
//! | [`scene`] | all of the above, over a composed tree | [`RenderScene`]: instances with node path, world matrix, shared `f32` buffers, materials, bounds |
//!
//! Each decoder takes the attribute's `serde_json::Value` as stored in
//! [`openbim_ifcx::IfcxNode::attributes`] and returns a [`DecodeError`] for
//! malformed values instead of panicking. [`PointCloud::from_attributes`] and
//! [`NodePresentation::from_attributes`] pick their attributes from anything
//! implementing [`Attributes`]. Coordinates stay `f64`; see [`math`].
//!
//! [`RenderScene::from_composition`] walks a composed node tree once and
//! returns the flat scene a viewer draws, with each shared geometry stored
//! once and instanced. Not yet implemented: GLB export.
//!
//! ```
//! use openbim_ifcx::IfcxFile;
//! use openbim_ifcx_geometry::{mesh, transform, Transform, TriangleMesh};
//!
//! let file = IfcxFile::from_json_str(r#"{
//!     "header": {"id": "demo", "ifcxVersion": "ifcx_alpha", "dataVersion": "1.0.0",
//!                "author": "someone", "timestamp": "2026-10-03"},
//!     "imports": [], "schemas": {},
//!     "data": [{"path": "slab", "attributes": {
//!         "usd::xformop": {"transform": [[1,0,0,0],[0,1,0,0],[0,0,1,0],[0,0,3,1]]},
//!         "usd::usdgeom::mesh": {"points": [[0,0,0],[1,0,0],[0,1,0]],
//!                                "faceVertexIndices": [0,1,2]}
//!     }}]
//! }"#)?;
//! let attributes = file.data[0].attributes.as_ref().unwrap();
//! let local = Transform::from_attribute(&attributes[transform::ATTRIBUTE])?;
//! let mesh = TriangleMesh::from_attribute(&attributes[mesh::ATTRIBUTE])?;
//! let world: Vec<_> = mesh.positions.iter().map(|&p| local.transform_point(p)).collect();
//! assert_eq!(world[1], [1.0, 0.0, 3.0]);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! Point clouds and presentation:
//!
//! ```
//! use openbim_ifcx_geometry::{is_visible, resolve_basic_material, NodePresentation, PointCloud};
//! use serde_json::json;
//!
//! let node = json!({
//!     "points::array": {"positions": [[0, 0, 0], [1, 0, 0]]},
//!     "bsi::ifc::presentation::diffuseColor": [1, 0, 0],
//! });
//! let attributes = node.as_object().unwrap();
//!
//! let cloud = PointCloud::from_attributes(attributes)?.unwrap();
//! assert_eq!(cloud.positions, [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]]);
//!
//! let own = NodePresentation::from_attributes(attributes)?;
//! assert!(is_visible(true, own.visibility));
//! assert_eq!(resolve_basic_material([&own]).color, [1.0, 0.0, 0.0]);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! A render scene from a composed file:
//!
//! ```
//! use openbim_ifcx::{compose, flatten, IfcxFile};
//! use openbim_ifcx_geometry::{GeometryKind, RenderScene, SceneOptions};
//!
//! let file = IfcxFile::from_json_str(r#"{
//!     "header": {"id": "demo", "ifcxVersion": "ifcx_alpha", "dataVersion": "1.0.0",
//!                "author": "someone", "timestamp": "2026-10-03"},
//!     "imports": [], "schemas": {},
//!     "data": [
//!         {"path": "storey", "children": {"Slab": "slab"}, "attributes": {
//!             "usd::xformop": {"transform": [[1,0,0,0],[0,1,0,0],[0,0,1,0],[0,0,3,1]]},
//!             "bsi::ifc::presentation::diffuseColor": [0.8, 0.2, 0.2]}},
//!         {"path": "slab", "attributes": {"usd::usdgeom::mesh": {
//!             "points": [[0,0,0],[1,0,0],[0,1,0]], "faceVertexIndices": [0,1,2]}}}
//!     ]
//! }"#)?;
//! let composition = compose(&flatten(&file.data))?;
//! let scene = RenderScene::from_composition(&composition, &SceneOptions::default());
//! let slab = &scene.instances[0];
//! assert_eq!(slab.path, "storey/Slab");
//! assert_eq!(slab.geometry.kind(), GeometryKind::Mesh);
//! assert_eq!(slab.world.translation(), [0.0, 0.0, 3.0]);
//! assert!(scene.warnings.is_empty());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

#![forbid(unsafe_code)]

mod attributes;
pub mod curves;
pub mod error;
mod json;
pub mod math;
pub mod mesh;
pub mod points;
pub mod presentation;
pub mod scene;
pub mod transform;

pub use attributes::Attributes;
pub use curves::{CurveGeometry, Polyline, UnsupportedCurve};
pub use error::DecodeError;
pub use math::Vec3;
pub use mesh::TriangleMesh;
pub use points::PointCloud;
pub use presentation::{
    is_visible, resolve_basic_material, resolve_mesh_material, AlphaMode, BasicMaterial,
    GltfMaterial, Material, NodePresentation, NormalTexture, OcclusionTexture, Visibility,
};
pub use scene::{
    Bounds, GeometryKind, GeometryRef, Instance, LineBuffer, MeshBuffer, Origin, PointBuffer,
    RenderScene, SceneOptions, SceneWarning, SceneWarningKind,
};
pub use transform::{world_from_parent, Transform};
