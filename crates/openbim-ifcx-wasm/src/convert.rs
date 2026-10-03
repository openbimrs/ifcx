//! JS arguments into binding-core inputs, and JSON text into JS objects.

use js_sys::{Array, Map, Object, Reflect, Uint8Array, JSON};
use openbim_ifcx_binding_core::{BindingError, GlbSettings, LayerSet};
use wasm_bindgen::{JsCast, JsValue};

/// The bytes of a file given as a string or a `Uint8Array`.
pub(crate) fn input_bytes(value: &JsValue, what: &str) -> Result<Vec<u8>, BindingError> {
    if let Some(text) = value.as_string() {
        return Ok(text.into_bytes());
    }
    if let Some(bytes) = value.dyn_ref::<Uint8Array>() {
        return Ok(bytes.to_vec());
    }
    Err(BindingError::InvalidArgument(format!(
        "{what} must be a string or a Uint8Array"
    )))
}

/// JSON text produced by the core as a plain JS value.
pub(crate) fn parse_json(text: &str) -> Result<JsValue, BindingError> {
    // The core only hands out JSON it wrote itself, so this cannot fail in
    // practice; a failure is still reported rather than unwrapped.
    JSON::parse(text).map_err(|_| BindingError::Write("result is not valid JSON".into()))
}

/// `layers` (one input or an array of them, weakest first) and the
/// `imports` of `options` (an object or a `Map` from import `uri` to input).
pub(crate) fn layer_set(
    layers: &JsValue,
    options: Option<&JsValue>,
) -> Result<LayerSet, BindingError> {
    let mut set = LayerSet::new();
    if Array::is_array(layers) {
        for (index, layer) in Array::from(layers).iter().enumerate() {
            set.push_layer(input_bytes(&layer, &format!("layers[{index}]"))?);
        }
    } else {
        set.push_layer(input_bytes(layers, "layers")?);
    }

    let Some(options) = options.filter(|o| !o.is_undefined() && !o.is_null()) else {
        return Ok(set);
    };
    if !options.is_object() {
        return Err(BindingError::InvalidArgument(
            "options must be an object".into(),
        ));
    }
    let imports = Reflect::get(options, &"imports".into())
        .map_err(|_| BindingError::InvalidArgument("cannot read options.imports".into()))?;
    if imports.is_undefined() || imports.is_null() {
        return Ok(set);
    }
    set.resolve_imports();
    let entries: Array = if let Some(map) = imports.dyn_ref::<Map>() {
        Array::from(&map.entries().into())
    } else if imports.is_object() && !Array::is_array(&imports) {
        Object::entries(imports.unchecked_ref())
    } else {
        return Err(BindingError::InvalidArgument(
            "options.imports must be an object or a Map from uri to file".into(),
        ));
    };
    for entry in entries.iter() {
        let entry = Array::from(&entry);
        let uri = entry.get(0).as_string().ok_or_else(|| {
            BindingError::InvalidArgument("options.imports keys must be strings".into())
        })?;
        let bytes = input_bytes(&entry.get(1), &format!("options.imports[{uri:?}]"))?;
        set.add_import(uri, bytes);
    }
    Ok(set)
}

/// `yUp` and `originOnRoot` of `options`, each `true` when absent.
pub(crate) fn glb_settings(options: Option<&JsValue>) -> Result<GlbSettings, BindingError> {
    let mut settings = GlbSettings::default();
    let Some(options) = options.filter(|o| o.is_object()) else {
        return Ok(settings);
    };
    let flag = |name: &str| -> Result<Option<bool>, BindingError> {
        let value = Reflect::get(options, &name.into())
            .map_err(|_| BindingError::InvalidArgument(format!("cannot read options.{name}")))?;
        if value.is_undefined() || value.is_null() {
            return Ok(None);
        }
        value.as_bool().map(Some).ok_or_else(|| {
            BindingError::InvalidArgument(format!("options.{name} must be a boolean"))
        })
    };
    if let Some(y_up) = flag("yUp")? {
        settings.y_up = y_up;
    }
    if let Some(origin_on_root) = flag("originOnRoot")? {
        settings.origin_on_root = origin_on_root;
    }
    Ok(settings)
}
