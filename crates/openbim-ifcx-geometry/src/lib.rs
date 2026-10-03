//! Renderer-neutral geometry and viewer helpers for IFC5 / IFCX.
//!
//! This crate will read a composed `openbim-ifcx` node tree and produce a flat
//! render scene: world transforms, triangle meshes, polylines, point clouds,
//! and presentation, plus GLB export of that scene. It never depends on a
//! renderer or GPU API (see `docs/adr/0002`).
//!
//! No extraction is implemented yet.

#![forbid(unsafe_code)]

/// The package's current, deliberately limited status.
pub const PACKAGE_STATUS: &str =
    "SCAFFOLD: no IFCX geometry extraction or export is implemented yet.";

#[cfg(test)]
mod tests {
    use super::PACKAGE_STATUS;

    #[test]
    fn status_is_explicitly_scaffold() {
        assert!(PACKAGE_STATUS.starts_with("SCAFFOLD: "));
    }

    #[test]
    fn builds_on_the_core_crate() {
        let _: fn(&str) -> Result<openbim_ifcx::IfcxFile, _> =
            openbim_ifcx::IfcxFile::from_json_str;
    }
}
