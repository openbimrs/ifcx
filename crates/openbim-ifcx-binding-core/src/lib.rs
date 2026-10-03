//! Host-independent core of the IFCX language bindings.
//!
//! The WebAssembly and Python bindings each wrap [`Document`] and
//! [`LayerSet`] and hand their results to the host as JSON text, which the
//! host parses into plain objects. The operations, the JSON shapes and the
//! error codes live here once, so the hosts cannot drift apart: a fix or a
//! new operation lands in one place and every binding exposes it.
//!
//! GLB export goes through `openbim-ifcx-geometry`: the composed layers
//! become a render scene, written by its `to_glb`.
//!
//! This crate adds no IFCX behaviour of its own. It is a thin, host-shaped
//! view of `openbim-ifcx`; if a binding needs more, that crate grows first.
//!
//! ```
//! use openbim_ifcx_binding_core::LayerSet;
//!
//! let layer = br#"{
//!     "header": {"id": "demo", "ifcxVersion": "ifcx_alpha", "dataVersion": "1.0.0",
//!                "author": "someone", "timestamp": "2026-10-03"},
//!     "imports": [], "schemas": {},
//!     "data": [{"path": "site", "attributes": {"demo::name": "Site"}}]
//! }"#;
//! let mut layers = LayerSet::new();
//! layers.push_layer(layer.to_vec());
//! let tree: serde_json::Value = serde_json::from_str(&layers.compose_json()?).unwrap();
//! assert_eq!(tree["children"]["site"]["attributes"]["demo::name"], "Site");
//! # Ok::<(), openbim_ifcx_binding_core::BindingError>(())
//! ```

#![forbid(unsafe_code)]

mod document;
mod error;
mod layers;
mod report;
mod tree;

pub use document::Document;
pub use error::BindingError;
pub use layers::{GlbSettings, LayerSet};

#[cfg(test)]
mod fixtures {
    //! The hand-written fixtures of `openbim-ifcx`, shared by the tests.

    macro_rules! fixture {
        ($name:literal) => {
            include_bytes!(concat!("../../openbim-ifcx/tests/fixtures/", $name)).as_slice()
        };
    }
    pub(crate) use fixture;
}
