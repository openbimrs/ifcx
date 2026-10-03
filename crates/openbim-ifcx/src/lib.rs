//! OpenBIM.rs family for IFC5 / IFCX.
//!
//! IFCX is a layered, componentised JSON model with USD-like composition. It
//! is not an EXPRESS schema release, so it lives here rather than as a
//! `SchemaVersion` in `openbim-ifc` (see `docs/adr/0001`).
//!
//! No reader, writer, or composition behavior is implemented yet.

#![forbid(unsafe_code)]

/// The package's current, deliberately limited status.
pub const PACKAGE_STATUS: &str =
    "SCAFFOLD: no IFCX reader, writer, or composition is implemented yet.";

#[cfg(test)]
mod tests {
    use super::PACKAGE_STATUS;

    #[test]
    fn status_is_explicitly_scaffold() {
        assert!(PACKAGE_STATUS.starts_with("SCAFFOLD: "));
    }
}
