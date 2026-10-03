//! JavaScript surface of the shared binding core.
//!
//! Each export converts JS arguments, calls the core, and converts the
//! result. No IFCX logic lives here.

use js_sys::Uint8Array;
use openbim_ifcx_binding_core::Document;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;

use crate::convert::{glb_settings, input_bytes, layer_set, parse_json};
use crate::error::js_error;

#[wasm_bindgen(typescript_custom_section)]
const TYPES: &'static str = r#"
/** An IFCX file as JSON text or as its UTF-8 bytes. */
export type IfcxInput = string | Uint8Array;

/** Options for `compose` and `validate`. */
export interface LayerOptions {
  /**
   * Files that imports name, keyed by the exact import `uri`. When given
   * (even empty), imports are resolved as upstream does and every import
   * must be here; without it, imports are not resolved.
   */
  imports?: Record<string, IfcxInput> | Map<string, IfcxInput>;
}

/** One node of a composed tree. The root has path `""`. */
export interface ComposedNode {
  path: string;
  attributes: Record<string, unknown>;
  children: Record<string, ComposedNode>;
}

/** The `kind` of a validation failure. */
export type ValidationFailureKind =
  | "missing-schema"
  | "unknown-inherited-schema"
  | "inheritance-cycle"
  | "type-mismatch"
  | "not-an-integer"
  | "not-an-option"
  | "missing-key"
  | "too-few-elements"
  | "too-many-elements"
  | "missing-restrictions"
  | "unknown-data-type"
  | "other";

/** One attribute value that does not match its schema. */
export interface ValidationFailure {
  /** Path of the node carrying the attribute. */
  node: string;
  /** Attribute id, which is also the schema id. */
  attribute: string;
  /** RFC 6901 JSON pointer into the value; `""` for the value itself. */
  pointer: string;
  kind: ValidationFailureKind;
  message: string;
}

export interface ValidationReport {
  valid: boolean;
  failures: ValidationFailure[];
}

/**
 * Compose layers, weakest first, into a tree of plain objects: the
 * artificial root (path `""`) over every root node.
 */
export function compose(layers: IfcxInput | IfcxInput[], options?: LayerOptions): ComposedNode;

/**
 * Check the attributes of the layers, weakest first, against the schemas of
 * every layer (and of resolved imports), after merging nodes that share a
 * path.
 */
export function validate(layers: IfcxInput | IfcxInput[], options?: LayerOptions): ValidationReport;

/** Options for `exportGlb`. */
export interface GlbOptions extends LayerOptions {
  /** Rotate IFCX's Z-up axes to glTF's Y-up. Default `true`. */
  yUp?: boolean;
  /**
   * Keep the model's coordinates by putting the scene origin on the root
   * node. With `false` the model sits around the glTF origin, which is then
   * only recorded in the root's `extras.ifcxOrigin`. Default `true`.
   */
  originOnRoot?: boolean;
}

/**
 * Compose layers, weakest first, and write their render scene as a binary
 * glTF 2.0 (`.glb`) file: transformed, instanced meshes, lines and points
 * with their materials, one glTF node per instance named by its IFCX path.
 */
export function exportGlb(layers: IfcxInput | IfcxInput[], options?: GlbOptions): Uint8Array;

/** The `code` of an `IfcxError`. */
export type IfcxErrorCode =
  | "read"
  | "write"
  | "layer"
  | "compose"
  | "invalid-argument"
  | "glb";
"#;

/// One IFCX file. Writing it back is lossless: key order, unknown fields,
/// `null` deletions and every number stay as read.
///
/// A newtype because `wasm_bindgen` can only export a type defined in this
/// crate; all state and behaviour are the core's.
#[wasm_bindgen]
#[derive(Debug)]
pub struct IfcxFile(Document);

#[wasm_bindgen]
impl IfcxFile {
    /// Read an IFCX file from its JSON text or UTF-8 bytes.
    #[wasm_bindgen(js_name = parse)]
    pub fn parse_js(
        #[wasm_bindgen(unchecked_param_type = "IfcxInput")] input: &JsValue,
    ) -> Result<IfcxFile, JsValue> {
        let bytes = input_bytes(input, "input").map_err(js_error)?;
        Document::parse(&bytes).map(IfcxFile).map_err(js_error)
    }

    /// The file as JSON text; `pretty` indents it by two spaces.
    #[wasm_bindgen(js_name = write)]
    pub fn write_js(&self, pretty: Option<bool>) -> Result<String, JsValue> {
        self.0.write(pretty.unwrap_or(false)).map_err(js_error)
    }

    /// The file as a plain object, so `JSON.stringify(file)` works. Unlike
    /// `write`, a plain object may reorder integer-like keys and round
    /// integers beyond 2^53.
    #[wasm_bindgen(js_name = toJSON, unchecked_return_type = "Record<string, unknown>")]
    pub fn to_json_js(&self) -> Result<JsValue, JsValue> {
        let text = self.0.write(false).map_err(js_error)?;
        parse_json(&text).map_err(js_error)
    }

    /// The `header` object.
    #[wasm_bindgen(getter, js_name = header, unchecked_return_type = "Record<string, unknown>")]
    pub fn header_js(&self) -> Result<JsValue, JsValue> {
        let text = self.0.header_json().map_err(js_error)?;
        parse_json(&text).map_err(js_error)
    }

    /// Number of entries in `data`; several may share a path.
    #[wasm_bindgen(getter, js_name = nodeCount)]
    pub fn node_count_js(&self) -> u32 {
        u32::try_from(self.0.node_count()).unwrap_or(u32::MAX)
    }

    /// Check every attribute against this file's own `schemas`. Imports are
    /// not resolved; use the `validate` function to include them.
    #[wasm_bindgen(js_name = validate, unchecked_return_type = "ValidationReport")]
    pub fn validate_js(&self) -> Result<JsValue, JsValue> {
        parse_json(&self.0.validate_json()).map_err(js_error)
    }
}

/// Compose layers, weakest first, into a tree of plain objects: the
/// artificial root (path `""`) over every root node. Declared for
/// TypeScript in `TYPES`, where `options` can be optional.
#[wasm_bindgen(js_name = compose, skip_typescript)]
pub fn compose_js(layers: &JsValue, options: Option<JsValue>) -> Result<JsValue, JsValue> {
    let set = layer_set(layers, options.as_ref()).map_err(js_error)?;
    let text = set.compose_json().map_err(js_error)?;
    parse_json(&text).map_err(js_error)
}

/// Check the attributes of the layers, weakest first, against the schemas
/// of every layer (and of resolved imports), after merging nodes that share
/// a path. Declared for TypeScript in `TYPES`.
#[wasm_bindgen(js_name = validate, skip_typescript)]
pub fn validate_js(layers: &JsValue, options: Option<JsValue>) -> Result<JsValue, JsValue> {
    let set = layer_set(layers, options.as_ref()).map_err(js_error)?;
    let text = set.validate_json().map_err(js_error)?;
    parse_json(&text).map_err(js_error)
}

/// Compose layers, weakest first, and write their render scene as GLB
/// bytes. Declared for TypeScript in `TYPES`.
#[wasm_bindgen(js_name = exportGlb, skip_typescript)]
pub fn export_glb_js(layers: &JsValue, options: Option<JsValue>) -> Result<Uint8Array, JsValue> {
    let set = layer_set(layers, options.as_ref()).map_err(js_error)?;
    let settings = glb_settings(options.as_ref()).map_err(js_error)?;
    let glb = set.to_glb(&settings).map_err(js_error)?;
    Ok(Uint8Array::from(glb.as_slice()))
}
