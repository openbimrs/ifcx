//! The error shared by every attribute decoder in this crate.

use std::fmt;

/// Why an attribute value could not be decoded into geometry.
///
/// `at` fields locate the offending value inside the attribute value, for
/// example `points[3][1]` or `faceVertexIndices[7]`.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum DecodeError {
    /// A value does not have the JSON shape the attribute requires.
    WrongShape {
        at: String,
        /// What was expected, for example `"array of 3 numbers"`.
        expected: &'static str,
    },
    /// A required field of an object-valued attribute is absent.
    MissingField { field: &'static str },
    /// A vertex index points past the end of the vertex list.
    IndexOutOfRange {
        at: String,
        index: u32,
        vertex_count: usize,
    },
    /// A triangle index list whose length is not a multiple of three.
    NotTriangles { index_count: usize },
    /// A face that is not a triangle, from `faceVertexCounts`.
    NonTriangleFace { face: usize, vertex_count: u64 },
    /// Per-face or per-curve vertex counts that do not add up to the
    /// number of indices or points they describe.
    CountMismatch {
        field: &'static str,
        counted: u64,
        available: usize,
    },
    /// A curve with fewer than two vertices, which cannot form a line.
    CurveTooShort { curve: usize, vertex_count: u64 },
    /// A 4×4 transform whose last column is not `(0, 0, 0, 1)`.
    NonAffineTransform { last_column: [f64; 4] },
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongShape { at, expected } => write!(f, "{at}: expected {expected}"),
            Self::MissingField { field } => write!(f, "missing field `{field}`"),
            Self::IndexOutOfRange {
                at,
                index,
                vertex_count,
            } => write!(
                f,
                "{at}: index {index} out of range for {vertex_count} vertices"
            ),
            Self::NotTriangles { index_count } => write!(
                f,
                "{index_count} face vertex indices is not a multiple of 3"
            ),
            Self::NonTriangleFace { face, vertex_count } => write!(
                f,
                "face {face} has {vertex_count} vertices; only triangles are supported"
            ),
            Self::CountMismatch {
                field,
                counted,
                available,
            } => write!(f, "`{field}` adds up to {counted}, but {available} exist"),
            Self::CurveTooShort {
                curve,
                vertex_count,
            } => write!(
                f,
                "curve {curve} has {vertex_count} vertices; a line needs at least 2"
            ),
            Self::NonAffineTransform { last_column } => write!(
                f,
                "transform last column is {last_column:?}, expected [0, 0, 0, 1]"
            ),
        }
    }
}

impl std::error::Error for DecodeError {}
