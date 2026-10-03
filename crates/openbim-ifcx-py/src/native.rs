//! The native classes and functions, wrapped by `openbim_ifcx` in Python.

use openbim_ifcx_binding_core::{Document, GlbSettings, LayerSet};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};

use crate::error::py_err;

/// Native half of `openbim_ifcx.IfcxFile`; not part of the public API.
///
/// Usable from any thread: the document is `Send + Sync` and the GIL
/// serialises calls.
#[pyclass(module = "openbim_ifcx._native", name = "NativeFile", frozen)]
pub struct NativeFile {
    inner: Document,
}

#[pymethods]
impl NativeFile {
    /// Read an IFCX file from its UTF-8 JSON bytes, releasing the GIL.
    #[staticmethod]
    fn parse(py: Python<'_>, data: &[u8]) -> PyResult<Self> {
        let owned = data.to_vec();
        let inner = py.detach(move || Document::parse(&owned)).map_err(py_err)?;
        Ok(Self { inner })
    }

    /// The file as JSON text; `pretty` indents it by two spaces.
    #[pyo3(signature = (pretty = false))]
    fn write(&self, pretty: bool) -> PyResult<String> {
        self.inner.write(pretty).map_err(py_err)
    }

    /// The `header` object as JSON text.
    fn header_json(&self) -> PyResult<String> {
        self.inner.header_json().map_err(py_err)
    }

    /// Number of entries in `data`.
    #[getter]
    fn node_count(&self) -> usize {
        self.inner.node_count()
    }

    /// The validation report against the file's own schemas, as JSON text.
    fn validate_json(&self) -> String {
        self.inner.validate_json()
    }
}

/// Layers and imports as a core `LayerSet`. The wrapper passes `bytes`
/// only; it converts `str` and other buffers first.
fn layer_set(
    layers: Vec<Bound<'_, PyBytes>>,
    imports: Option<Bound<'_, PyDict>>,
) -> PyResult<LayerSet> {
    let mut set = LayerSet::new();
    for layer in &layers {
        set.push_layer(layer.as_bytes().to_vec());
    }
    if let Some(imports) = imports {
        set.resolve_imports();
        for (uri, bytes) in imports.iter() {
            let uri: String = uri.extract()?;
            let bytes = bytes.cast_into::<PyBytes>()?;
            set.add_import(uri, bytes.as_bytes().to_vec());
        }
    }
    Ok(set)
}

/// The composed tree of `layers` (weakest first) as JSON text, releasing
/// the GIL while composing.
#[pyfunction]
#[pyo3(signature = (layers, imports = None))]
pub fn compose_json(
    py: Python<'_>,
    layers: Vec<Bound<'_, PyBytes>>,
    imports: Option<Bound<'_, PyDict>>,
) -> PyResult<String> {
    let set = layer_set(layers, imports)?;
    py.detach(move || set.compose_json()).map_err(py_err)
}

/// The validation report of `layers` (weakest first) as JSON text,
/// releasing the GIL while validating.
#[pyfunction]
#[pyo3(signature = (layers, imports = None))]
pub fn validate_json(
    py: Python<'_>,
    layers: Vec<Bound<'_, PyBytes>>,
    imports: Option<Bound<'_, PyDict>>,
) -> PyResult<String> {
    let set = layer_set(layers, imports)?;
    py.detach(move || set.validate_json()).map_err(py_err)
}

/// The composed `layers` (weakest first) as GLB bytes, releasing the GIL
/// while composing and writing.
#[pyfunction]
#[pyo3(signature = (layers, imports = None, y_up = true, origin_on_root = true))]
pub fn export_glb<'py>(
    py: Python<'py>,
    layers: Vec<Bound<'py, PyBytes>>,
    imports: Option<Bound<'py, PyDict>>,
    y_up: bool,
    origin_on_root: bool,
) -> PyResult<Bound<'py, PyBytes>> {
    let set = layer_set(layers, imports)?;
    let settings = GlbSettings {
        y_up,
        origin_on_root,
    };
    let glb = py.detach(move || set.to_glb(&settings)).map_err(py_err)?;
    Ok(PyBytes::new(py, &glb))
}
