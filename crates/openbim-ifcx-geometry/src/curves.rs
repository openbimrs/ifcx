//! Polylines from `usd::usdgeom::basiscurves`.
//!
//! In `ifcx_alpha` files the attribute is an object with `points` and,
//! optionally, `curveVertexCounts`, which splits the points into several
//! curves:
//!
//! ```json
//! "usd::usdgeom::basiscurves": {
//!     "points": [[0, 0, 0], [10, 0, 0], [0, 5, 0], [0, 5, 3]],
//!     "curveVertexCounts": [2, 2]
//! }
//! ```
//!
//! None of the 85 curves in the upstream examples (`1a63082`) has a `type`,
//! `basis`, or `wrap` field, and upstream's viewer draws every one as a
//! straight polyline. This crate does the same: a missing `type` means
//! linear. That differs from USD, whose schema default is `cubic`. A `type`
//! other than `"linear"` yields [`CurveGeometry::Unsupported`] instead of a
//! polyline that would misrepresent the curve. For linear curves `basis` is
//! irrelevant and ignored; `wrap: "periodic"` closes each polyline, and
//! `"nonperiodic"` or `"pinned"` leave it open.

use serde_json::Value;

use crate::error::DecodeError;
use crate::json;
use crate::math::Vec3;

/// Attribute id that carries curves.
pub const ATTRIBUTE: &str = "usd::usdgeom::basiscurves";

/// One connected line strip in the node's local coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct Polyline {
    /// Vertices in order; at least two.
    pub points: Vec<Vec3>,
    /// Whether the last vertex connects back to the first.
    pub closed: bool,
}

/// A curve type this crate does not turn into polylines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedCurve {
    /// The `type` field, for example `"cubic"`.
    pub curve_type: String,
    /// The `basis` field, if present, for example `"bspline"`.
    pub basis: Option<String>,
}

/// The result of decoding a curves attribute that is well formed.
#[derive(Debug, Clone, PartialEq)]
pub enum CurveGeometry {
    /// Linear curves, one polyline per curve.
    Polylines(Vec<Polyline>),
    /// Curves of a type that is not drawn yet. Skip them rather than draw
    /// their control points as lines.
    Unsupported(UnsupportedCurve),
}

impl CurveGeometry {
    /// Decodes the value of a [`ATTRIBUTE`] (`usd::usdgeom::basiscurves`)
    /// attribute.
    ///
    /// ```
    /// use openbim_ifcx_geometry::curves::CurveGeometry;
    ///
    /// let value = serde_json::json!({"points": [[0, 0, 0], [10, 0, 0]]});
    /// let CurveGeometry::Polylines(lines) = CurveGeometry::from_attribute(&value)? else {
    ///     unreachable!()
    /// };
    /// assert_eq!(lines[0].points, [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0]]);
    /// # Ok::<(), openbim_ifcx_geometry::DecodeError>(())
    /// ```
    pub fn from_attribute(value: &Value) -> Result<Self, DecodeError> {
        let object = json::object(value, ATTRIBUTE)?;
        let curve_type = json::optional_str(object, "type")?;
        let basis = json::optional_str(object, "basis")?;
        let wrap = json::optional_str(object, "wrap")?;
        let points = json::points3(json::field(object, "points")?, "points")?;

        if let Some(t) = curve_type.filter(|t| *t != "linear") {
            return Ok(Self::Unsupported(UnsupportedCurve {
                curve_type: t.to_owned(),
                basis: basis.map(str::to_owned),
            }));
        }
        let closed = match wrap {
            None | Some("nonperiodic") | Some("pinned") => false,
            Some("periodic") => true,
            Some(_) => {
                return Err(DecodeError::WrongShape {
                    at: "wrap".to_owned(),
                    expected: "\"nonperiodic\", \"periodic\", or \"pinned\"",
                })
            }
        };

        let counts = match object.get("curveVertexCounts") {
            Some(v) => json::counts(v, "curveVertexCounts")?,
            None if points.is_empty() => Vec::new(),
            None => vec![points.len() as u64],
        };
        let counted = counts.iter().try_fold(0u64, |sum, &n| sum.checked_add(n));
        if counted != Some(points.len() as u64) {
            return Err(DecodeError::CountMismatch {
                field: "curveVertexCounts",
                counted: counted.unwrap_or(u64::MAX),
                available: points.len(),
            });
        }

        let mut polylines = Vec::with_capacity(counts.len());
        let mut rest = points.as_slice();
        for (curve, &n) in counts.iter().enumerate() {
            if n < 2 {
                return Err(DecodeError::CurveTooShort {
                    curve,
                    vertex_count: n,
                });
            }
            // The sum check above guarantees `n` fits.
            let (head, tail) = rest.split_at(n as usize);
            rest = tail;
            polylines.push(Polyline {
                points: head.to_vec(),
                closed,
            });
        }
        Ok(Self::Polylines(polylines))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn polylines(v: Value) -> Vec<Polyline> {
        match CurveGeometry::from_attribute(&v).unwrap() {
            CurveGeometry::Polylines(p) => p,
            other => panic!("expected polylines, got {other:?}"),
        }
    }

    #[test]
    fn linear_curve_yields_its_vertices() {
        let lines = polylines(json!({"points": [[0, 0, 0], [10, 0, 0], [10, 0.5, 3]]}));
        assert_eq!(
            lines,
            vec![Polyline {
                points: vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [10.0, 0.5, 3.0]],
                closed: false
            }]
        );
    }

    #[test]
    fn explicit_linear_type_is_accepted() {
        let lines = polylines(json!({
            "type": "linear", "basis": "bezier", "points": [[0, 0, 0], [1, 0, 0]]
        }));
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn curve_vertex_counts_split_curves() {
        let lines = polylines(json!({
            "points": [[0, 0, 0], [1, 0, 0], [2, 0, 0], [0, 5, 0], [0, 5, 3]],
            "curveVertexCounts": [3, 2]
        }));
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].points.len(), 3);
        assert_eq!(lines[1].points, vec![[0.0, 5.0, 0.0], [0.0, 5.0, 3.0]]);
    }

    #[test]
    fn no_points_means_no_curves() {
        assert!(polylines(json!({"points": []})).is_empty());
    }

    #[test]
    fn periodic_wrap_closes() {
        let lines = polylines(json!({
            "wrap": "periodic", "points": [[0, 0, 0], [1, 0, 0], [0, 1, 0]]
        }));
        assert!(lines[0].closed);
    }

    #[test]
    fn non_linear_types_are_unsupported() {
        let g = CurveGeometry::from_attribute(&json!({
            "type": "cubic", "basis": "bspline",
            "points": [[0, 0, 0], [1, 1, 0], [2, 0, 0], [3, 1, 0]]
        }))
        .unwrap();
        assert_eq!(
            g,
            CurveGeometry::Unsupported(UnsupportedCurve {
                curve_type: "cubic".into(),
                basis: Some("bspline".into())
            })
        );
    }

    #[test]
    fn malformed_curves_are_errors() {
        let err = |v: Value| CurveGeometry::from_attribute(&v).unwrap_err();
        assert_eq!(
            err(json!({"curveVertexCounts": [2]})),
            DecodeError::MissingField { field: "points" }
        );
        assert_eq!(
            err(json!({"points": [[0, 0, 0], [1, 0, 0]], "curveVertexCounts": [3]})),
            DecodeError::CountMismatch {
                field: "curveVertexCounts",
                counted: 3,
                available: 2
            }
        );
        assert_eq!(
            err(json!({"points": [[0, 0, 0], [1, 0, 0], [2, 0, 0]], "curveVertexCounts": [2, 1]})),
            DecodeError::CurveTooShort {
                curve: 1,
                vertex_count: 1
            }
        );
        assert_eq!(
            err(json!({"points": [[0, 0, 0]]})),
            DecodeError::CurveTooShort {
                curve: 0,
                vertex_count: 1
            }
        );
        assert_eq!(
            err(json!({"points": [[0, 0, 0], [1, 0]]})),
            DecodeError::WrongShape {
                at: "points[1]".into(),
                expected: "array of 3 numbers"
            }
        );
        assert!(matches!(
            err(json!({"type": 3, "points": []})),
            DecodeError::WrongShape { .. }
        ));
        assert!(matches!(
            err(json!({"wrap": "spiral", "points": [[0, 0, 0], [1, 0, 0]]})),
            DecodeError::WrongShape { .. }
        ));
    }
}
