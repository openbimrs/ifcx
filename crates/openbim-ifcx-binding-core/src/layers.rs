//! Several layers composed into one model.
//!
//! A [`LayerSet`] holds the layers a host passed, weakest first, and
//! optionally the files their `imports` name. Without imports, the layers'
//! schemas and data are concatenated in order (upstream's `Federate`), as the
//! `compose-json` example of `openbim-ifcx` does: the last layer wins.
//!
//! With imports, the layers become the imports of a synthetic main layer, as
//! upstream's `ifcx compose` command builds it, and the stack is resolved
//! through a [`MemoryResolver`] holding the given files under their exact
//! import `uri`. In upstream order an import overrides the layer importing
//! it, so a layer's imports override that layer, and the next layer
//! overrides both. Every import must then be present; `integrity` values are
//! checked against the given bytes.

use openbim_ifcx::layers::{federate, LayerStackBuilder, MemoryResolver};
use openbim_ifcx::{compose, flatten, Composition, IfcxFile, IfcxHeader, ImportNode};
use openbim_ifcx_geometry::glb::{to_glb, GlbOptions};
use openbim_ifcx_geometry::scene::{RenderScene, SceneOptions};

use crate::error::BindingError;
use crate::{report, tree};

/// Key of the synthetic main layer. Import keys are URIs, which never
/// contain `<`.
const MAIN_KEY: &str = "<main>";

/// Key of the `index`th layer passed by the host.
fn layer_key(index: usize) -> String {
    format!("<layer {index}>")
}

/// How [`LayerSet::to_glb`] places the model; see
/// `openbim_ifcx_geometry::glb::GlbOptions`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlbSettings {
    /// Rotate IFCX's Z-up axes to glTF's Y-up, so the model stands upright
    /// in glTF viewers. Default `true`.
    pub y_up: bool,
    /// Keep the model's coordinates by putting the scene origin on the root
    /// node. With `false` the model sits around the glTF origin, and the
    /// origin is only recorded in the root's `extras`. Default `true`.
    pub origin_on_root: bool,
}

impl Default for GlbSettings {
    fn default() -> Self {
        Self {
            y_up: true,
            origin_on_root: true,
        }
    }
}

/// The layers of one model and, optionally, the files they import.
#[derive(Debug, Clone, Default)]
pub struct LayerSet {
    layers: Vec<Vec<u8>>,
    imports: Option<Vec<(String, Vec<u8>)>>,
}

impl LayerSet {
    /// An empty set that does not resolve imports.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a layer's UTF-8 JSON bytes on top of the layers already added.
    pub fn push_layer(&mut self, bytes: Vec<u8>) {
        self.layers.push(bytes);
    }

    /// Resolves imports, even if no import file is added: every import of
    /// every layer must then be one of the added files.
    pub fn resolve_imports(&mut self) {
        self.imports.get_or_insert_with(Vec::new);
    }

    /// Serves `bytes` for imports whose `uri` is exactly `uri`, and turns on
    /// import resolution.
    pub fn add_import(&mut self, uri: String, bytes: Vec<u8>) {
        self.imports.get_or_insert_with(Vec::new).push((uri, bytes));
    }

    /// Number of layers added.
    pub fn len(&self) -> usize {
        self.layers.len()
    }

    /// Whether no layer was added.
    pub fn is_empty(&self) -> bool {
        self.layers.is_empty()
    }

    /// All layers merged into one file. Without imports, the layers' schemas
    /// and data are concatenated in order. With imports, the layers are the
    /// imports of a synthetic main layer, resolved in upstream order, so a
    /// layer's imports override it and the next layer overrides both; the
    /// first layer's header leads the result.
    pub fn federate(&self) -> Result<IfcxFile, BindingError> {
        if self.layers.is_empty() {
            return Err(BindingError::InvalidArgument(
                "at least one layer is needed".into(),
            ));
        }
        match &self.imports {
            None => {
                let files = self
                    .layers
                    .iter()
                    .enumerate()
                    .map(|(index, bytes)| {
                        IfcxFile::from_json_slice(bytes)
                            .map_err(|e| BindingError::Read(format!("layer {index}: {e}")))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(federate(&files).expect("at least one layer"))
            }
            Some(imports) => self.federate_stack(imports),
        }
    }

    fn federate_stack(&self, imports: &[(String, Vec<u8>)]) -> Result<IfcxFile, BindingError> {
        let mut resolver = MemoryResolver::new();
        for (uri, bytes) in imports {
            resolver.insert(uri.clone(), bytes.clone());
        }
        let mut main_imports = Vec::with_capacity(self.layers.len());
        for (index, bytes) in self.layers.iter().enumerate() {
            // Read each layer here as well, so a malformed one is a `read`
            // error naming its index, not a `layer` error naming a key the
            // host never saw.
            IfcxFile::from_json_slice(bytes)
                .map_err(|e| BindingError::Read(format!("layer {index}: {e}")))?;
            resolver.insert(layer_key(index), bytes.clone());
            main_imports.push(ImportNode {
                uri: layer_key(index),
                integrity: None,
                extra: Default::default(),
            });
        }
        let main = IfcxFile {
            header: IfcxHeader {
                id: MAIN_KEY.into(),
                ifcx_version: "ifcx_alpha".into(),
                data_version: "1.0.0".into(),
                author: String::new(),
                timestamp: String::new(),
                extra: Default::default(),
            },
            imports: main_imports,
            schemas: Default::default(),
            data: Vec::new(),
            extra: Default::default(),
        };
        resolver
            .insert_file(&main)
            .map_err(|e| BindingError::Write(e.to_string()))?;
        let stack = LayerStackBuilder::new(resolver)
            .build(MAIN_KEY)
            .map_err(|e| BindingError::Layer(e.to_string()))?;
        // The synthetic main layer has no schemas and no data, but its header
        // would lead the federated file; the first real layer's leads instead.
        let mut file = federate(stack.layers()[1..].iter().map(|layer| layer.file()))
            .expect("the main layer imports at least one layer");
        file.header = stack.layers()[1].file().header.clone();
        Ok(file)
    }

    /// Flattens and composes the federated layers.
    pub fn compose(&self) -> Result<Composition, BindingError> {
        let file = self.federate()?;
        compose(&flatten(&file.data)).map_err(|e| BindingError::Compose(e.to_string()))
    }

    /// The composed tree as JSON text: the artificial root over every root
    /// node, each node `{"path", "attributes", "children"}`.
    pub fn compose_json(&self) -> Result<String, BindingError> {
        let composition = self.compose()?;
        serde_json::to_string(&tree::Tree(&composition.root()))
            .map_err(|e| BindingError::Write(e.to_string()))
    }

    /// The composed layers as a binary glTF 2.0 (`.glb`) file: their flat
    /// render scene with transformed, instanced meshes, lines and points and
    /// their materials, one glTF node per instance named by its IFCX path.
    ///
    /// Geometry values that do not decode are left out of the scene, as
    /// `RenderScene` does; the file is still written.
    pub fn to_glb(&self, settings: &GlbSettings) -> Result<Vec<u8>, BindingError> {
        let composition = self.compose()?;
        let scene = RenderScene::from_composition(&composition, &SceneOptions::default());
        let options = GlbOptions::default()
            .with_y_up(settings.y_up)
            .with_origin_on_root(settings.origin_on_root);
        to_glb(&scene, &options).map_err(|e| BindingError::Glb(e.to_string()))
    }

    /// Checks the attributes of the federated layers against their merged
    /// `schemas`, and returns the report as JSON text:
    /// `{"valid": bool, "failures": [{"node", "attribute", "pointer", "kind",
    /// "message"}]}`. `kind` is a stable code such as `missing-schema` or
    /// `type-mismatch`.
    pub fn validate_json(&self) -> Result<String, BindingError> {
        let file = self.federate()?;
        Ok(report::to_json(&file.validate().err().unwrap_or_default()))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{GlbSettings, LayerSet};
    use crate::fixtures::fixture;

    fn tree(layers: &LayerSet) -> Value {
        serde_json::from_str(&layers.compose_json().unwrap()).unwrap()
    }

    fn set(layers: &[&[u8]]) -> LayerSet {
        let mut set = LayerSet::new();
        for layer in layers {
            set.push_layer(layer.to_vec());
        }
        set
    }

    #[test]
    fn one_layer_composes_to_the_upstream_tree() {
        let composed = tree(&set(&[fixture!("occurrence-type.ifcx")]));
        let expected: Value =
            serde_json::from_slice(fixture!("occurrence-type.composed.json")).unwrap();
        assert_eq!(composed, expected);
    }

    /// A layer that renames the roof of `geometry-model.ifcx` and, when
    /// `imports` is not empty, imports those URIs.
    fn overlay(imports: &[&str]) -> Vec<u8> {
        let imports: Vec<String> = imports
            .iter()
            .map(|uri| format!(r#"{{"uri": "{uri}"}}"#))
            .collect();
        format!(
            r#"{{
                "header": {{"id": "overlay", "ifcxVersion": "ifcx_alpha", "dataVersion": "1.0.0",
                           "author": "test", "timestamp": "2026-10-03"}},
                "imports": [{}], "schemas": {{}},
                "data": [{{"path": "roof", "attributes": {{"example::class": "Roof"}}}}]
            }}"#,
            imports.join(", ")
        )
        .into_bytes()
    }

    fn roof_class(tree: &Value) -> &Value {
        &tree["children"]["pavilion"]["children"]["Storey"]["children"]["Roof"]["attributes"]
            ["example::class"]
    }

    #[test]
    fn the_last_layer_wins() {
        let model = fixture!("geometry-model.ifcx");
        assert_eq!(roof_class(&tree(&set(&[model]))), "Slab");
        assert_eq!(roof_class(&tree(&set(&[model, &overlay(&[])]))), "Roof");
        assert_eq!(roof_class(&tree(&set(&[&overlay(&[]), model]))), "Slab");
        // The overlay's attribute matches the model's schema.
        let report: Value =
            serde_json::from_str(&set(&[model, &overlay(&[])]).validate_json().unwrap()).unwrap();
        assert_eq!(report["valid"], true, "{report}");
    }

    #[test]
    fn imports_resolve_from_memory_and_override_their_importer() {
        // In upstream order an import overrides the layer importing it.
        let mut layers = set(&[&overlay(&["model.ifcx"])]);
        layers.add_import(
            "model.ifcx".into(),
            fixture!("geometry-model.ifcx").to_vec(),
        );
        assert_eq!(roof_class(&tree(&layers)), "Slab");

        // A later layer overrides an earlier one and everything it imports.
        let mut layers = set(&[&overlay(&["model.ifcx"]), &overlay(&[])]);
        layers.add_import(
            "model.ifcx".into(),
            fixture!("geometry-model.ifcx").to_vec(),
        );
        assert_eq!(roof_class(&tree(&layers)), "Roof");
        // The imported schemas validate the overlay's attribute.
        let report: Value = serde_json::from_str(&layers.validate_json().unwrap()).unwrap();
        assert_eq!(report["valid"], true, "{report}");
    }

    #[test]
    fn a_chain_resolves_with_integrity_in_upstream_order() {
        let mut layers = set(&[fixture!("layers/chain/main.ifcx")]);
        layers.add_import(
            "mid.ifcx".into(),
            fixture!("layers/chain/mid.ifcx").to_vec(),
        );
        layers.add_import(
            "sub/base.ifcx".into(),
            fixture!("layers/chain/sub/base.ifcx").to_vec(),
        );
        let file = layers.federate().unwrap();
        assert_eq!(file.header.id, "openbimrs/ifcx/fixtures/layers/chain/main");
        let ids = |name: &str| -> usize {
            let layer: Value = serde_json::from_slice(match name {
                "main" => fixture!("layers/chain/main.ifcx"),
                "mid" => fixture!("layers/chain/mid.ifcx"),
                _ => fixture!("layers/chain/sub/base.ifcx"),
            })
            .unwrap();
            layer["data"].as_array().unwrap().len()
        };
        assert_eq!(file.data.len(), ids("main") + ids("mid") + ids("base"));
        // Schemas from every layer of the stack are merged for validation.
        let report: Value = serde_json::from_str(&layers.validate_json().unwrap()).unwrap();
        assert_eq!(report["valid"], true, "{report}");

        // Without imports, only the given layer's data and schemas count.
        let alone = set(&[fixture!("layers/chain/main.ifcx")])
            .federate()
            .unwrap();
        assert_eq!(alone.data.len(), ids("main"));
    }

    #[test]
    fn a_missing_import_or_a_wrong_integrity_is_a_layer_error() {
        let mut layers = set(&[fixture!("layers/chain/main.ifcx")]);
        layers.resolve_imports();
        let error = layers.compose_json().unwrap_err();
        assert_eq!(error.code(), "layer");
        assert!(error.to_string().contains("mid.ifcx"), "{error}");

        let mut layers = set(&[fixture!("layers/chain/mid.ifcx")]);
        layers.add_import(
            "sub/base.ifcx".into(),
            fixture!("layers/chain/main.ifcx").to_vec(),
        );
        let error = layers.compose_json().unwrap_err();
        assert_eq!(error.code(), "layer");
        assert!(error.to_string().contains("integrity"), "{error}");
    }

    #[test]
    fn refusals_carry_stable_codes() {
        assert_eq!(
            LayerSet::new().compose_json().unwrap_err().code(),
            "invalid-argument"
        );
        let error = set(&[fixture!("minimal.ifcx"), b"[]"])
            .compose_json()
            .unwrap_err();
        assert_eq!(error.code(), "read");
        assert!(error.to_string().starts_with("layer 1:"), "{error}");
        let mut with_imports = set(&[b"nope"]);
        with_imports.resolve_imports();
        assert_eq!(with_imports.compose_json().unwrap_err().code(), "read");
        // layer-edit alone references nodes only layer-base defines.
        let error = set(&[fixture!("layer-edit.ifcx")])
            .compose_json()
            .unwrap_err();
        assert_eq!(error.code(), "compose");
    }

    /// The JSON chunk of a GLB file, after checking the container.
    fn glb_json(glb: &[u8]) -> Value {
        assert_eq!(&glb[..4], b"glTF");
        assert_eq!(u32::from_le_bytes(glb[4..8].try_into().unwrap()), 2);
        assert_eq!(
            u32::from_le_bytes(glb[8..12].try_into().unwrap()) as usize,
            glb.len()
        );
        assert_eq!(glb.len() % 4, 0);
        let len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
        assert_eq!(&glb[16..20], b"JSON");
        serde_json::from_slice(&glb[20..20 + len]).unwrap()
    }

    #[test]
    fn layers_export_as_glb_with_one_node_per_instance() {
        let model = fixture!("geometry-model.ifcx");
        let glb = set(&[model]).to_glb(&GlbSettings::default()).unwrap();
        let json = glb_json(&glb);
        let names: Vec<&str> = json["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|node| node["name"].as_str())
            .collect();
        assert!(
            names.contains(&"pavilion/Storey/Column 1/Body"),
            "{names:?}"
        );
        assert!(names.contains(&"pavilion/Storey/Roof"), "{names:?}");
        // Column 2 hides its inherited axis.
        assert!(
            !names.contains(&"pavilion/Storey/Column 2/Axis"),
            "{names:?}"
        );
        assert!(json["nodes"][0]["rotation"].is_array(), "Y-up by default");

        let z_up = set(&[model])
            .to_glb(&GlbSettings {
                y_up: false,
                origin_on_root: false,
            })
            .unwrap();
        let root = &glb_json(&z_up)["nodes"][0];
        assert!(root.get("rotation").is_none() && root.get("translation").is_none());

        // An overlay layer changes the export like it changes the tree.
        let hidden = br#"{
            "header": {"id": "hide", "ifcxVersion": "ifcx_alpha", "dataVersion": "1.0.0",
                       "author": "test", "timestamp": "2026-10-03"},
            "imports": [], "schemas": {},
            "data": [{"path": "roof", "attributes":
                {"usd::usdgeom::visibility": {"visibility": "invisible"}}}]
        }"#;
        let json = glb_json(
            &set(&[model, hidden])
                .to_glb(&GlbSettings::default())
                .unwrap(),
        );
        assert!(!json["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["name"] == "pavilion/Storey/Roof"));
    }

    #[test]
    fn a_model_without_geometry_is_an_empty_glb_and_errors_keep_their_codes() {
        glb_json(
            &set(&[fixture!("minimal.ifcx")])
                .to_glb(&GlbSettings::default())
                .unwrap(),
        );
        let error = set(&[fixture!("layer-edit.ifcx")])
            .to_glb(&GlbSettings::default())
            .unwrap_err();
        assert_eq!(error.code(), "compose");
        assert_eq!(
            LayerSet::new()
                .to_glb(&GlbSettings::default())
                .unwrap_err()
                .code(),
            "invalid-argument"
        );
    }

    #[test]
    fn federated_validation_merges_schemas_in_layer_order() {
        let report: Value = serde_json::from_str(
            &set(&[fixture!("invalid-attributes.ifcx")])
                .validate_json()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(report["valid"], false);
        assert!(!report["failures"].as_array().unwrap().is_empty());
    }
}
