//! Shared readers for the JSON shapes geometry attributes use.

use serde_json::{Map, Value};

use crate::error::DecodeError;
use crate::math::Vec3;

pub(crate) fn object<'a>(
    value: &'a Value,
    attribute: &str,
) -> Result<&'a Map<String, Value>, DecodeError> {
    value.as_object().ok_or_else(|| DecodeError::WrongShape {
        at: attribute.to_owned(),
        expected: "object",
    })
}

pub(crate) fn field<'a>(
    object: &'a Map<String, Value>,
    field: &'static str,
) -> Result<&'a Value, DecodeError> {
    object.get(field).ok_or(DecodeError::MissingField { field })
}

pub(crate) fn array<'a>(value: &'a Value, at: &str) -> Result<&'a [Value], DecodeError> {
    value
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| DecodeError::WrongShape {
            at: at.to_owned(),
            expected: "array",
        })
}

pub(crate) fn number(value: &Value, at: impl FnOnce() -> String) -> Result<f64, DecodeError> {
    value.as_f64().ok_or_else(|| DecodeError::WrongShape {
        at: at(),
        expected: "number",
    })
}

/// `[[x, y, z], ...]`, integers and floats alike.
pub(crate) fn points3(value: &Value, field: &str) -> Result<Vec<Vec3>, DecodeError> {
    let items = array(value, field)?;
    let mut points = Vec::with_capacity(items.len());
    for (i, item) in items.iter().enumerate() {
        let xyz = match item.as_array() {
            Some(xyz) if xyz.len() == 3 => xyz,
            _ => {
                return Err(DecodeError::WrongShape {
                    at: format!("{field}[{i}]"),
                    expected: "array of 3 numbers",
                })
            }
        };
        let mut p = [0.0; 3];
        for (j, (c, v)) in p.iter_mut().zip(xyz).enumerate() {
            *c = number(v, || format!("{field}[{i}][{j}]"))?;
        }
        points.push(p);
    }
    Ok(points)
}

/// Non-negative integers that fit in `u32`.
pub(crate) fn indices(value: &Value, field: &str) -> Result<Vec<u32>, DecodeError> {
    let items = array(value, field)?;
    items
        .iter()
        .enumerate()
        .map(|(i, v)| {
            v.as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .ok_or_else(|| DecodeError::WrongShape {
                    at: format!("{field}[{i}]"),
                    expected: "integer from 0 to 4294967295",
                })
        })
        .collect()
}

/// Non-negative integer counts.
pub(crate) fn counts(value: &Value, field: &str) -> Result<Vec<u64>, DecodeError> {
    let items = array(value, field)?;
    items
        .iter()
        .enumerate()
        .map(|(i, v)| {
            v.as_u64().ok_or_else(|| DecodeError::WrongShape {
                at: format!("{field}[{i}]"),
                expected: "non-negative integer",
            })
        })
        .collect()
}

pub(crate) fn optional_str<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<Option<&'a str>, DecodeError> {
    match object.get(field) {
        None => Ok(None),
        Some(v) => v.as_str().map(Some).ok_or_else(|| DecodeError::WrongShape {
            at: field.to_owned(),
            expected: "string",
        }),
    }
}
