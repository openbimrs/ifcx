//! Indexed triangle meshes from `usd::usdgeom::mesh`.
//!
//! In `ifcx_alpha` files the attribute is an object:
//!
//! ```json
//! "usd::usdgeom::mesh": {
//!     "points": [[0, 0, 0], [1, 0, 0], [1, 1, 0]],
//!     "faceVertexIndices": [0, 1, 2]
//! }
//! ```
//!
//! Every face is a triangle: none of the 1 925 meshes in the upstream examples
//! (`1a63082`) has `faceVertexCounts`, and upstream's viewer reads the indices
//! three at a time. If `faceVertexCounts` is present, every entry must be 3;
//! polygons are rejected rather than triangulated, since a fan would draw
//! concave faces wrongly. Other fields (USD `orientation`, normals, primvars)
//! are ignored; winding is taken as USD's default right-handed,
//! counter-clockwise front face.

use serde_json::Value;

use crate::error::DecodeError;
use crate::json;
use crate::math::{cross, normalize_or_zero, sub, Vec3};

/// Attribute id that carries a mesh.
pub const ATTRIBUTE: &str = "usd::usdgeom::mesh";

/// An indexed triangle mesh in the node's local coordinates.
///
/// Positions are `f64`; see [`crate::math`] for why.
#[derive(Debug, Clone, PartialEq)]
pub struct TriangleMesh {
    /// Vertex positions, from `points`.
    pub positions: Vec<Vec3>,
    /// Three indices into `positions` per triangle, from
    /// `faceVertexIndices`. Every index is in range.
    pub indices: Vec<u32>,
    /// One unit normal per triangle, `(b - a) × (c - a)` normalised, so
    /// `face_normals.len() == indices.len() / 3`. Degenerate triangles get
    /// a zero vector. Renderers that want flat shading duplicate vertices
    /// per face and use these.
    pub face_normals: Vec<Vec3>,
}

impl TriangleMesh {
    /// Decodes the value of a [`ATTRIBUTE`] (`usd::usdgeom::mesh`)
    /// attribute.
    ///
    /// ```
    /// use openbim_ifcx_geometry::mesh::TriangleMesh;
    ///
    /// let value = serde_json::json!({
    ///     "points": [[0, 0, 0], [1, 0, 0], [0, 1, 0]],
    ///     "faceVertexIndices": [0, 1, 2]
    /// });
    /// let mesh = TriangleMesh::from_attribute(&value)?;
    /// assert_eq!(mesh.triangle_count(), 1);
    /// assert_eq!(mesh.face_normals[0], [0.0, 0.0, 1.0]);
    /// # Ok::<(), openbim_ifcx_geometry::DecodeError>(())
    /// ```
    pub fn from_attribute(value: &Value) -> Result<Self, DecodeError> {
        let object = json::object(value, ATTRIBUTE)?;
        let positions = json::points3(json::field(object, "points")?, "points")?;
        let indices = json::indices(
            json::field(object, "faceVertexIndices")?,
            "faceVertexIndices",
        )?;
        if let Some(counts) = object.get("faceVertexCounts") {
            let counts = json::counts(counts, "faceVertexCounts")?;
            if let Some((face, &n)) = counts.iter().enumerate().find(|(_, &n)| n != 3) {
                return Err(DecodeError::NonTriangleFace {
                    face,
                    vertex_count: n,
                });
            }
            let counted = counts.len() as u64 * 3;
            if counted != indices.len() as u64 {
                return Err(DecodeError::CountMismatch {
                    field: "faceVertexCounts",
                    counted,
                    available: indices.len(),
                });
            }
        }
        Self::new(positions, indices)
    }

    /// Builds a mesh from positions and triangle indices, checking the
    /// indices and computing face normals.
    pub fn new(positions: Vec<Vec3>, indices: Vec<u32>) -> Result<Self, DecodeError> {
        if indices.len() % 3 != 0 {
            return Err(DecodeError::NotTriangles {
                index_count: indices.len(),
            });
        }
        if let Some((i, &index)) = indices
            .iter()
            .enumerate()
            .find(|(_, &index)| index as usize >= positions.len())
        {
            return Err(DecodeError::IndexOutOfRange {
                at: format!("faceVertexIndices[{i}]"),
                index,
                vertex_count: positions.len(),
            });
        }
        let face_normals = indices
            .chunks_exact(3)
            .map(|t| {
                let [a, b, c] = [0, 1, 2].map(|k| positions[t[k] as usize]);
                normalize_or_zero(cross(sub(b, a), sub(c, a)))
            })
            .collect();
        Ok(Self {
            positions,
            indices,
            face_normals,
        })
    }

    /// Number of triangles.
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }

    /// Triangles as index triples.
    pub fn triangles(&self) -> impl Iterator<Item = [u32; 3]> + '_ {
        self.indices.chunks_exact(3).map(|t| [t[0], t[1], t[2]])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Unit cube, 8 corners, 12 outward-facing counter-clockwise triangles.
    fn cube() -> Value {
        json!({
            "points": [
                [0, 0, 0], [1, 0, 0], [1, 1, 0], [0, 1, 0],
                [0, 0, 1], [1, 0, 1], [1, 1, 1], [0, 1, 1]
            ],
            "faceVertexIndices": [
                0, 2, 1, 0, 3, 2,
                4, 5, 6, 4, 6, 7,
                0, 1, 5, 0, 5, 4,
                1, 2, 6, 1, 6, 5,
                2, 3, 7, 2, 7, 6,
                3, 0, 4, 3, 4, 7
            ]
        })
    }

    #[test]
    fn cube_has_8_positions_and_36_indices() {
        let mesh = TriangleMesh::from_attribute(&cube()).unwrap();
        assert_eq!(mesh.positions.len(), 8);
        assert_eq!(mesh.indices.len(), 36);
        assert_eq!(mesh.triangle_count(), 12);
        assert_eq!(mesh.face_normals.len(), 12);
        let expected = [
            [0.0, 0.0, -1.0],
            [0.0, 0.0, 1.0],
            [0.0, -1.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [-1.0, 0.0, 0.0],
        ];
        for (i, n) in mesh.face_normals.iter().enumerate() {
            assert_eq!(*n, expected[i / 2], "triangle {i}");
        }
        assert_eq!(mesh.triangles().next(), Some([0, 2, 1]));
    }

    #[test]
    fn accepts_integer_and_float_coordinates() {
        let mesh = TriangleMesh::from_attribute(&json!({
            "points": [[0, 0.5, 0], [2, 0, 0], [0, 0, 1e3]],
            "faceVertexIndices": [0, 1, 2]
        }))
        .unwrap();
        assert_eq!(mesh.positions[2], [0.0, 0.0, 1000.0]);
    }

    #[test]
    fn degenerate_triangle_has_zero_normal() {
        let mesh = TriangleMesh::new(vec![[0.0; 3], [1.0, 0.0, 0.0]], vec![0, 1, 1]).unwrap();
        assert_eq!(mesh.face_normals, vec![[0.0; 3]]);
    }

    #[test]
    fn out_of_range_index_is_an_error() {
        let e = TriangleMesh::from_attribute(&json!({
            "points": [[0, 0, 0], [1, 0, 0], [0, 1, 0]],
            "faceVertexIndices": [0, 1, 2, 2, 1, 3]
        }))
        .unwrap_err();
        assert_eq!(
            e,
            DecodeError::IndexOutOfRange {
                at: "faceVertexIndices[5]".into(),
                index: 3,
                vertex_count: 3
            }
        );
    }

    #[test]
    fn non_triangle_index_list_is_an_error() {
        let e = TriangleMesh::from_attribute(&json!({
            "points": [[0, 0, 0], [1, 0, 0], [1, 1, 0], [0, 1, 0]],
            "faceVertexIndices": [0, 1, 2, 3]
        }))
        .unwrap_err();
        assert_eq!(e, DecodeError::NotTriangles { index_count: 4 });
    }

    #[test]
    fn polygon_face_counts_are_rejected() {
        let e = TriangleMesh::from_attribute(&json!({
            "points": [[0, 0, 0], [1, 0, 0], [1, 1, 0], [0, 1, 0]],
            "faceVertexIndices": [0, 1, 2, 3, 0, 2],
            "faceVertexCounts": [4, 2]
        }))
        .unwrap_err();
        assert_eq!(
            e,
            DecodeError::NonTriangleFace {
                face: 0,
                vertex_count: 4
            }
        );
    }

    #[test]
    fn triangle_face_counts_must_cover_indices() {
        let ok = TriangleMesh::from_attribute(&json!({
            "points": [[0, 0, 0], [1, 0, 0], [0, 1, 0]],
            "faceVertexIndices": [0, 1, 2],
            "faceVertexCounts": [3]
        }));
        assert!(ok.is_ok());
        let e = TriangleMesh::from_attribute(&json!({
            "points": [[0, 0, 0], [1, 0, 0], [0, 1, 0]],
            "faceVertexIndices": [0, 1, 2, 0, 1, 2],
            "faceVertexCounts": [3]
        }))
        .unwrap_err();
        assert_eq!(
            e,
            DecodeError::CountMismatch {
                field: "faceVertexCounts",
                counted: 3,
                available: 6
            }
        );
    }

    #[test]
    fn wrong_shapes_are_errors() {
        let err = |v: Value| TriangleMesh::from_attribute(&v).unwrap_err();
        assert_eq!(
            err(json!({"faceVertexIndices": []})),
            DecodeError::MissingField { field: "points" }
        );
        assert_eq!(
            err(json!({"points": []})),
            DecodeError::MissingField {
                field: "faceVertexIndices"
            }
        );
        assert_eq!(
            err(json!({"points": [[0, 0]], "faceVertexIndices": []})),
            DecodeError::WrongShape {
                at: "points[0]".into(),
                expected: "array of 3 numbers"
            }
        );
        assert_eq!(
            err(json!({"points": [[0, 0, 0]], "faceVertexIndices": [0, -1, 0]})),
            DecodeError::WrongShape {
                at: "faceVertexIndices[1]".into(),
                expected: "integer from 0 to 4294967295"
            }
        );
        assert_eq!(
            err(json!({"points": [[0, 0, 0]], "faceVertexIndices": [0, 0.5, 0]})),
            DecodeError::WrongShape {
                at: "faceVertexIndices[1]".into(),
                expected: "integer from 0 to 4294967295"
            }
        );
    }

    #[test]
    fn empty_mesh_is_valid() {
        let mesh =
            TriangleMesh::from_attribute(&json!({"points": [], "faceVertexIndices": []})).unwrap();
        assert_eq!(mesh.triangle_count(), 0);
    }
}
