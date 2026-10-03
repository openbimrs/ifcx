//! OpenBIM.rs family for IFC5 / IFCX.
//!
//! IFCX is a layered, componentised JSON model with USD-like composition. It
//! is not an EXPRESS schema release, so it lives here rather than as a
//! `SchemaVersion` in `openbim-ifc` (see `docs/adr/0001`).
//!
//! This crate reads and writes single `ifcx_alpha` files losslessly, composes
//! layered nodes into a resolved tree ([`flatten`], then [`compose()`]; see
//! [`compose`](mod@compose)), and checks attribute values against a file's
//! `schemas` ([`IfcxFile::validate`], [`validate_flat`],
//! [`validate_attributes`]; rules in [`validate`](mod@validate)). [`layers`]
//! loads a file with its imports through a caller-supplied resolver,
//! validates the stack as a whole
//! ([`LayerStack::validate`](layers::LayerStack::validate)), and federates
//! it into one file, ready to flatten and compose.
//!
//! [`flatten_owned`] and [`layers::LayerStack::into_federated`] consume
//! their input and move attribute values instead of copying them; use them
//! when the nodes are not needed afterwards, which is the usual case after
//! reading a file:
//!
//! ```
//! use openbim_ifcx::{compose, flatten_owned, IfcxFile};
//!
//! let file = IfcxFile::from_json_str(r#"{
//!     "header": {"id": "demo", "ifcxVersion": "ifcx_alpha", "dataVersion": "1.0.0",
//!                "author": "someone", "timestamp": "2026-10-03"},
//!     "imports": [], "schemas": {},
//!     "data": [{"path": "site", "attributes": {"demo::name": "Site"}}]
//! }"#)?;
//! let composition = compose(&flatten_owned(file.data))?;
//! assert_eq!(composition.roots(), ["site"]);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Features
//!
//! - `integrity` (default): check SHA-2 `integrity` values on imports. Without
//!   it, an import that carries `integrity` is rejected rather than loaded
//!   unchecked.
//! - `fs`: [`layers::FsResolver`], which loads imports from local files. The
//!   crate does no filesystem access otherwise and never uses the network.
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

pub mod compose;
mod json;
pub mod layers;
mod model;
pub mod validate;

pub use compose::{
    compose, flatten, flatten_owned, ComposeError, ComposedNode, Composition, FlatNode,
};
pub use json::{ReadError, ReadErrorKind, WriteError};
pub use model::{
    ArrayRestrictions, DataType, EnumRestrictions, Extra, IfcxFile, IfcxHeader, IfcxNode,
    IfcxSchema, IfcxValueDescription, ImportNode, ObjectRestrictions,
};
pub use validate::{
    validate_attributes, validate_flat, validate_nodes, FailureKind, JsonType, ValidationFailure,
    ValidationReport, INTERNAL_ATTRIBUTE_PREFIX,
};
