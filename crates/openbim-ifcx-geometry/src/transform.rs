//! Local and world transforms from `usd::xformop`.
//!
//! # Matrix layout
//!
//! In `ifcx_alpha` files the attribute is an object with one field,
//! `transform`, holding four rows of four numbers:
//!
//! ```json
//! "usd::xformop": {"transform": [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [5, 0, 2, 1]]}
//! ```
//!
//! This is USD's convention: points are **row vectors** multiplied on the
//! left, `p' = [x, y, z, 1] · M`, so the translation is the **last row** and
//! the last column is `(0, 0, 0, 1)`. Every one of the 41 840 transforms in
//! the upstream examples (`1a63082`) has that last column. Upstream's viewer
//! (`src/viewer/render.ts`) loads the rows with three.js `Matrix4.set` and
//! then transposes, which yields the same mapping.
//!
//! Flattening the rows in file order gives the column-major element order
//! of the equivalent column-vector matrix, which is what glTF `node.matrix`
//! and three.js `Matrix4.elements` expect; see
//! [`Transform::to_column_major`].
//!
//! A node's transform places its own geometry and all of its children
//! relative to its parent. In the row-vector convention a child's world
//! matrix is `local · parent_world`; [`world_from_parent`] computes it, and
//! a node without the attribute inherits its parent's world matrix. Units
//! are those of the file (metres in every upstream sample); no scaling is
//! applied.

use serde_json::Value;

use crate::error::DecodeError;
use crate::json;
use crate::math::Vec3;

/// Attribute id that carries a node's local transform.
pub const ATTRIBUTE: &str = "usd::xformop";

/// An affine 4×4 transform in USD's row-vector layout.
///
/// Values are `f64` so that georeferenced translations keep millimetre
/// precision. The last column is always `(0, 0, 0, 1)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transform {
    rows: [[f64; 4]; 4],
}

impl Default for Transform {
    fn default() -> Self {
        Self::IDENTITY
    }
}

impl Transform {
    /// The transform that leaves every point where it is.
    pub const IDENTITY: Self = Self {
        rows: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };

    /// Builds a transform from rows in the file's layout (translation in
    /// the last row). Fails if the last column is not `(0, 0, 0, 1)` or a
    /// value is not finite.
    pub fn from_rows(rows: [[f64; 4]; 4]) -> Result<Self, DecodeError> {
        for (i, row) in rows.iter().enumerate() {
            for (j, v) in row.iter().enumerate() {
                if !v.is_finite() {
                    return Err(DecodeError::WrongShape {
                        at: format!("transform[{i}][{j}]"),
                        expected: "finite number",
                    });
                }
            }
        }
        let last_column = [rows[0][3], rows[1][3], rows[2][3], rows[3][3]];
        if last_column != [0.0, 0.0, 0.0, 1.0] {
            return Err(DecodeError::NonAffineTransform { last_column });
        }
        Ok(Self { rows })
    }

    /// A pure translation.
    pub fn from_translation(t: Vec3) -> Self {
        let mut rows = Self::IDENTITY.rows;
        rows[3] = [t[0], t[1], t[2], 1.0];
        Self { rows }
    }

    /// Decodes the value of a [`ATTRIBUTE`] (`usd::xformop`) attribute, an
    /// object whose `transform` field holds the matrix.
    ///
    /// ```
    /// use openbim_ifcx_geometry::transform::Transform;
    ///
    /// let value = serde_json::json!({"transform": [
    ///     [1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [5, 0, 2, 1]
    /// ]});
    /// let t = Transform::from_attribute(&value)?;
    /// assert_eq!(t.translation(), [5.0, 0.0, 2.0]);
    /// assert_eq!(t.transform_point([1.0, 1.0, 1.0]), [6.0, 1.0, 3.0]);
    /// # Ok::<(), openbim_ifcx_geometry::DecodeError>(())
    /// ```
    pub fn from_attribute(value: &Value) -> Result<Self, DecodeError> {
        let object = json::object(value, ATTRIBUTE)?;
        Self::from_matrix_json(json::field(object, "transform")?)
    }

    /// Decodes the matrix itself: four arrays of four numbers, rows first.
    pub fn from_matrix_json(value: &Value) -> Result<Self, DecodeError> {
        let shape_error = || DecodeError::WrongShape {
            at: "transform".to_owned(),
            expected: "array of 4 rows of 4 numbers",
        };
        let rows_json = json::array(value, "transform")?;
        if rows_json.len() != 4 {
            return Err(shape_error());
        }
        let mut rows = [[0.0; 4]; 4];
        for (i, (row, row_json)) in rows.iter_mut().zip(rows_json).enumerate() {
            let cells = match row_json.as_array() {
                Some(cells) if cells.len() == 4 => cells,
                _ => {
                    return Err(DecodeError::WrongShape {
                        at: format!("transform[{i}]"),
                        expected: "array of 4 numbers",
                    })
                }
            };
            for (j, (cell, v)) in row.iter_mut().zip(cells).enumerate() {
                *cell = json::number(v, || format!("transform[{i}][{j}]"))?;
            }
        }
        Self::from_rows(rows)
    }

    /// The rows in the file's layout; the translation is `rows()[3]`.
    pub fn rows(&self) -> &[[f64; 4]; 4] {
        &self.rows
    }

    /// The translation part.
    pub fn translation(&self) -> Vec3 {
        let r = self.rows[3];
        [r[0], r[1], r[2]]
    }

    /// The 16 values in column-major order of the equivalent column-vector
    /// matrix: the layout of glTF `node.matrix` and three.js
    /// `Matrix4.elements`. This equals the file's rows flattened in order.
    pub fn to_column_major(&self) -> [f64; 16] {
        let mut out = [0.0; 16];
        for (i, row) in self.rows.iter().enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(row);
        }
        out
    }

    /// The transform that applies `self` first and then `outer`; in the
    /// row-vector layout this is the product `self · outer`.
    ///
    /// For a node with local transform `local` under a parent with world
    /// transform `parent`, the node's world transform is
    /// `local.then(&parent)`.
    pub fn then(&self, outer: &Transform) -> Transform {
        let a = &self.rows;
        let b = &outer.rows;
        let mut rows = [[0.0; 4]; 4];
        for (i, row) in rows.iter_mut().enumerate() {
            for (j, cell) in row.iter_mut().enumerate() {
                *cell = (0..4).map(|k| a[i][k] * b[k][j]).sum();
            }
        }
        // Keep the affine column exact rather than accumulating rounding.
        for row in rows.iter_mut().take(3) {
            row[3] = 0.0;
        }
        rows[3][3] = 1.0;
        Transform { rows }
    }

    /// Maps a point, including the translation.
    pub fn transform_point(&self, p: Vec3) -> Vec3 {
        let v = self.transform_vector(p);
        let t = self.rows[3];
        [v[0] + t[0], v[1] + t[1], v[2] + t[2]]
    }

    /// Maps a direction, ignoring the translation. Normals need the inverse
    /// transpose instead when the transform scales unevenly.
    pub fn transform_vector(&self, v: Vec3) -> Vec3 {
        let m = &self.rows;
        [
            v[0] * m[0][0] + v[1] * m[1][0] + v[2] * m[2][0],
            v[0] * m[0][1] + v[1] * m[1][1] + v[2] * m[2][1],
            v[0] * m[0][2] + v[1] * m[1][2] + v[2] * m[2][2],
        ]
    }

    /// Determinant of the 3×3 linear part. A negative value means the
    /// transform mirrors, which reverses triangle winding.
    pub fn determinant(&self) -> f64 {
        let m = &self.rows;
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    }
}

/// World transform of a node from its parent's world transform and its own
/// local transform, if it has one. A node without a local transform inherits
/// its parent's world transform unchanged. Use [`Transform::IDENTITY`] as the
/// parent of a root.
pub fn world_from_parent(parent_world: &Transform, local: Option<&Transform>) -> Transform {
    match local {
        Some(local) => local.then(parent_world),
        None => *parent_world,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn close(a: Vec3, b: Vec3) -> bool {
        a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1e-12)
    }

    /// 90° about +Z: x → y, y → -x (row-vector layout).
    fn rot_z90() -> Transform {
        Transform::from_rows([
            [0.0, 1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ])
        .unwrap()
    }

    #[test]
    fn decodes_translation_from_last_row() {
        let t = Transform::from_attribute(&json!({"transform": [
            [1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [1.5, -2, 3.25, 1]
        ]}))
        .unwrap();
        assert_eq!(t.translation(), [1.5, -2.0, 3.25]);
        assert_eq!(t.transform_point([0.0; 3]), [1.5, -2.0, 3.25]);
        assert_eq!(t.transform_vector([1.0, 0.0, 0.0]), [1.0, 0.0, 0.0]);
    }

    #[test]
    fn column_major_matches_file_order() {
        let t = Transform::from_translation([7.0, 8.0, 9.0]);
        let m = t.to_column_major();
        assert_eq!(&m[12..16], &[7.0, 8.0, 9.0, 1.0]);
        assert_eq!(m[0], 1.0);
    }

    #[test]
    fn rotation_maps_rows_as_images_of_axes() {
        let r = rot_z90();
        assert!(close(r.transform_point([1.0, 0.0, 0.0]), [0.0, 1.0, 0.0]));
        assert!(close(r.transform_point([0.0, 1.0, 0.0]), [-1.0, 0.0, 0.0]));
        assert_eq!(r.determinant(), 1.0);
    }

    #[test]
    fn three_level_hierarchy() {
        // root: translate (10, 0, 0)
        // └─ a: rotate 90° about Z
        //    └─ b: no transform
        //       └─ c: translate (1, 0, 0)
        let root = world_from_parent(
            &Transform::IDENTITY,
            Some(&Transform::from_translation([10.0, 0.0, 0.0])),
        );
        let a = world_from_parent(&root, Some(&rot_z90()));
        let b = world_from_parent(&a, None);
        let c = world_from_parent(&b, Some(&Transform::from_translation([1.0, 0.0, 0.0])));

        assert_eq!(b, a, "a node without a transform inherits");
        // c's origin: (1,0,0) → rotate → (0,1,0) → translate → (10,1,0)
        assert!(close(c.translation(), [10.0, 1.0, 0.0]));
        assert!(close(c.transform_point([0.0, 1.0, 0.0]), [9.0, 1.0, 0.0]));
        assert_eq!(
            c.rows(),
            &[
                [0.0, 1.0, 0.0, 0.0],
                [-1.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [10.0, 1.0, 0.0, 1.0],
            ]
        );
    }

    #[test]
    fn then_is_associative_with_point_application() {
        let a = rot_z90();
        let b = Transform::from_translation([3.0, 4.0, 5.0]);
        let p = [1.0, 2.0, 3.0];
        assert!(close(
            a.then(&b).transform_point(p),
            b.transform_point(a.transform_point(p))
        ));
    }

    #[test]
    fn rejects_wrong_shapes() {
        let err = |v: Value| Transform::from_attribute(&v).unwrap_err();
        assert_eq!(
            err(json!([[1, 0, 0, 0]])),
            DecodeError::WrongShape {
                at: "usd::xformop".into(),
                expected: "object"
            }
        );
        assert_eq!(
            err(json!({})),
            DecodeError::MissingField { field: "transform" }
        );
        assert!(matches!(
            err(json!({"transform": [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0]]})),
            DecodeError::WrongShape { .. }
        ));
        assert_eq!(
            err(json!({"transform": [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1], [0, 0, 0, 1]]})),
            DecodeError::WrongShape {
                at: "transform[2]".into(),
                expected: "array of 4 numbers"
            }
        );
        assert_eq!(
            err(json!({"transform": [[1, 0, 0, 0], [0, 1, "x", 0], [0, 0, 1, 0], [0, 0, 0, 1]]})),
            DecodeError::WrongShape {
                at: "transform[1][2]".into(),
                expected: "number"
            }
        );
    }

    #[test]
    fn rejects_translation_in_last_column() {
        // Column-vector layout written by mistake.
        let e = Transform::from_attribute(&json!({"transform": [
            [1, 0, 0, 5], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]
        ]}))
        .unwrap_err();
        assert_eq!(
            e,
            DecodeError::NonAffineTransform {
                last_column: [5.0, 0.0, 0.0, 1.0]
            }
        );
    }
}
