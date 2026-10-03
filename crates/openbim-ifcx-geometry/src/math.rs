//! Small vector helpers shared by the decoders.
//!
//! Coordinates are `f64` throughout. IFCX files carry georeferenced
//! coordinates (upstream samples reach several million metres), where `f32`
//! keeps only about half a metre of precision. Convert to `f32` for a
//! renderer only after world transforms have been applied and, if needed, a
//! local origin has been subtracted.

/// A point or direction in 3D, `[x, y, z]`.
pub type Vec3 = [f64; 3];

pub(crate) fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(crate) fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

/// Unit vector in the direction of `v`, or zero if `v` has no length.
pub(crate) fn normalize_or_zero(v: Vec3) -> Vec3 {
    let len = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if len > 0.0 && len.is_finite() {
        [v[0] / len, v[1] / len, v[2] / len]
    } else {
        [0.0; 3]
    }
}
