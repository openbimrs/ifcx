//! Every attribute decoder on one arbitrary JSON value: mesh, curves,
//! transform, presentation, and `points::array`, with the invariants their
//! results promise.
#![no_main]

use libfuzzer_sys::fuzz_target;
use openbim_ifcx_geometry::presentation::GltfMaterial;
use openbim_ifcx_geometry::{
    CurveGeometry, NodePresentation, PointCloud, Transform, TriangleMesh, Visibility,
};
use serde_json::Value;

fuzz_target!(|data: &[u8]| {
    let Ok(value) = serde_json::from_slice::<Value>(data) else {
        return;
    };
    if let Ok(mesh) = TriangleMesh::from_attribute(&value) {
        assert_eq!(mesh.indices.len() % 3, 0);
        assert_eq!(mesh.face_normals.len(), mesh.triangle_count());
        assert!(mesh
            .indices
            .iter()
            .all(|&i| (i as usize) < mesh.positions.len()));
    }
    if let Ok(CurveGeometry::Polylines(lines)) = CurveGeometry::from_attribute(&value) {
        assert!(lines.iter().all(|l| l.points.len() >= 2));
    }
    if let Ok(t) = Transform::from_attribute(&value) {
        assert!(t.rows().iter().flatten().all(|v| v.is_finite()));
        let _ = t.transform_point([1.0, 2.0, 3.0]);
        let _ = t.determinant();
    }
    let _ = Transform::from_matrix_json(&value);
    let _ = Visibility::from_value(&value);
    let _ = GltfMaterial::from_value(&value);
    if let Some(object) = value.as_object() {
        let _ = NodePresentation::from_attributes(object);
        let _ = PointCloud::from_attributes(object);
    }
    if let Ok(cloud) = PointCloud::from_points_array(&value) {
        if let Some(colors) = &cloud.colors {
            assert_eq!(colors.len(), cloud.len());
        }
    }
});
