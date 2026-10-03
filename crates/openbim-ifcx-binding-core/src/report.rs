//! Validation reports as plain JSON.
//!
//! ```json
//! {"valid": false, "failures": [
//!   {"node": "wall", "attribute": "example::height", "pointer": "",
//!    "kind": "type-mismatch", "message": "expected Real, found string"}
//! ]}
//! ```
//!
//! `kind` is a stable code, like the error codes: one may be added, never
//! renamed. `message` is for people and may change.

use openbim_ifcx::{FailureKind, ValidationReport};
use serde_json::{json, Value};

/// The stable code of a failure kind.
fn kind_code(kind: &FailureKind) -> &'static str {
    match kind {
        FailureKind::MissingSchema => "missing-schema",
        FailureKind::UnknownInheritedSchema { .. } => "unknown-inherited-schema",
        FailureKind::InheritanceCycle { .. } => "inheritance-cycle",
        FailureKind::TypeMismatch { .. } => "type-mismatch",
        FailureKind::NotAnInteger { .. } => "not-an-integer",
        FailureKind::NotAnOption { .. } => "not-an-option",
        FailureKind::MissingKey { .. } => "missing-key",
        FailureKind::TooFewElements { .. } => "too-few-elements",
        FailureKind::TooManyElements { .. } => "too-many-elements",
        FailureKind::MissingRestrictions { .. } => "missing-restrictions",
        FailureKind::UnknownDataType { .. } => "unknown-data-type",
        // FailureKind is non-exhaustive; a kind added there shows up here
        // until it gets a code of its own.
        _ => "other",
    }
}

/// `report` as JSON text in the shape documented above.
pub(crate) fn to_json(report: &ValidationReport) -> String {
    let failures: Vec<Value> = report
        .failures
        .iter()
        .map(|failure| {
            json!({
                "node": failure.node,
                "attribute": failure.attribute,
                "pointer": failure.pointer,
                "kind": kind_code(&failure.kind),
                "message": failure.kind.to_string(),
            })
        })
        .collect();
    json!({"valid": report.is_valid(), "failures": failures}).to_string()
}
