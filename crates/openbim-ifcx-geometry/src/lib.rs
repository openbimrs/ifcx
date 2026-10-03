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
//!
//! Each decoder takes the attribute's `serde_json::Value` as stored in
//! [`openbim_ifcx::IfcxNode::attributes`] and returns a [`DecodeError`] for
//! malformed values instead of panicking. [`PointCloud::from_attributes`] and
//! [`NodePresentation::from_attributes`] pick their attributes from anything
//! implementing [`Attributes`]. Coordinates stay `f64`; see [`math`].
//!
//! Not yet implemented: walking a composed node tree to collect world
//! transforms, geometry, and presentation into a flat render scene, and GLB
//! export.
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

#![forbid(unsafe_code)]

mod attributes;
pub mod curves;
pub mod error;
mod json;
pub mod math;
pub mod mesh;
pub mod points;
pub mod presentation;
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
pub use transform::{world_from_parent, Transform};
