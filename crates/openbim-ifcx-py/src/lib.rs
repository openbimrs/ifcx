//! Python bindings for `openbim-ifcx`.
//!
//! The compiled module is `openbim_ifcx._native`; the public API is the pure
//! Python package `openbim_ifcx`, which wraps it. Results cross as JSON text
//! written by `openbim-ifcx-binding-core`, which the package turns into
//! dicts and lists with `json.loads`.
//!
//! This crate adds calling-convention glue only. IFCX behaviour belongs in
//! `openbim-ifcx`, shared binding behaviour in `openbim-ifcx-binding-core`.

#![forbid(unsafe_code)]

mod error;
mod native;

use pyo3::prelude::*;

/// The `openbim_ifcx._native` extension module.
#[pymodule]
fn _native(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_class::<native::NativeFile>()?;
    module.add_function(wrap_pyfunction!(native::compose_json, module)?)?;
    module.add_function(wrap_pyfunction!(native::validate_json, module)?)?;
    module.add_function(wrap_pyfunction!(native::export_glb, module)?)?;
    module.add("IfcxError", module.py().get_type::<error::IfcxError>())?;
    Ok(())
}
