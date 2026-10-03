//! OpenBIM.rs family for IFC5 / IFCX.
//!
//! IFCX is a layered, componentised JSON model with USD-like composition. It
//! is not an EXPRESS schema release, so it lives here rather than as a
//! `SchemaVersion` in `openbim-ifc` (see `docs/adr/0001`).
//!
//! This crate reads and writes single `ifcx_alpha` files losslessly. Layer
//! composition, imports, and schema validation are not implemented yet.
//!
//! ```
//! use openbim_ifcx::IfcxFile;
//!
//! let file = IfcxFile::from_json_str(r#"{
//!     "header": {"id": "demo", "ifcxVersion": "ifcx_alpha", "dataVersion": "1.0.0",
//!                "author": "someone", "timestamp": "2026-10-03"},
//!     "imports": [],
//!     "schemas": {},
//!     "data": [{"path": "a1", "children": {"Wall": "b2", "Old": null}}]
//! }"#)?;
//! let children = file.data[0].children.as_ref().unwrap();
//! assert_eq!(children["Wall"].as_deref(), Some("b2"));
//! assert_eq!(children["Old"], None);
//! # Ok::<(), openbim_ifcx::ReadError>(())
//! ```

#![forbid(unsafe_code)]

mod json;
mod model;

pub use json::{ReadError, ReadErrorKind, WriteError};
pub use model::{
    ArrayRestrictions, DataType, EnumRestrictions, Extra, IfcxFile, IfcxHeader, IfcxNode,
    IfcxSchema, IfcxValueDescription, ImportNode, ObjectRestrictions,
};
