//! Presentation from hand-written `ifcx_alpha` samples.

use openbim_ifcx::IfcxFile;
use openbim_ifcx_geometry::{
    is_visible, resolve_basic_material, resolve_mesh_material, AlphaMode, BasicMaterial,
    DecodeError, GltfMaterial, Material, NodePresentation, NormalTexture, OcclusionTexture,
    Visibility,
};
use serde_json::{json, Value};

/// A small layer: a storey (blue glass), a wall with a PBR material under
/// it, the wall's body, and a hidden opening with a child.
const LAYER: &str = r#"{
  "header": {"id": "presentation", "ifcxVersion": "ifcx_alpha", "dataVersion": "1",
             "author": "test", "timestamp": "2026-10-03"},
  "imports": [],
  "schemas": {},
  "data": [
    {"path": "storey", "children": {"Wall": "wall", "Opening": "opening"},
     "attributes": {"bsi::ifc::presentation::diffuseColor": [0.2, 0.4, 0.8],
                    "bsi::ifc::presentation::opacity": 0.3}},
    {"path": "wall", "children": {"Body": "body"},
     "attributes": {"gltf::material": {
        "pbrMetallicRoughness": {"baseColorFactor": [1, 0.9, 0.9, 1],
                                 "baseColorTexture": "textures/brick.webp",
                                 "metallicFactor": 0.2, "roughnessFactor": 0.7},
        "normalTexture": {"texture": "textures/brick-normal.webp", "scale": 0.5},
        "occlusionTexture": {"texture": "textures/brick-ao.webp"},
        "emissiveFactor": [0.5, 0.5, 1.0],
        "alphaMode": "MASK", "alphaCutoff": 0.25, "doubleSided": true}}},
    {"path": "body", "attributes": {"bsi::ifc::presentation::diffuseColor": [1, 1, 1]}},
    {"path": "opening", "children": {"Frame": "frame"},
     "attributes": {"usd::usdgeom::visibility": {"visibility": "invisible"}}},
    {"path": "frame", "attributes": {"usd::usdgeom::visibility": {"visibility": "inherited"}}}
  ]
}"#;

fn presentation(path: &str) -> NodePresentation {
    let file = IfcxFile::from_json_str(LAYER).unwrap();
    let node = file.data.iter().find(|n| n.path == path).unwrap();
    NodePresentation::from_attributes(node).unwrap()
}

#[test]
fn flattened_layers_decode_the_last_opinion() {
    // A second opinion on the body overrides its colour, as a later layer would.
    let mut file = IfcxFile::from_json_str(LAYER).unwrap();
    let mut edit = file.data.iter().find(|n| n.path == "body").unwrap().clone();
    edit.attributes.as_mut().unwrap().insert(
        "bsi::ifc::presentation::diffuseColor".into(),
        json!([0, 0, 0]),
    );
    file.data.push(edit);
    let flat = openbim_ifcx::flatten(&file.data);
    let body = NodePresentation::from_attributes(&flat["body"]).unwrap();
    assert_eq!(body.diffuse_color, Some([0.0; 3]));
}

/// Collects `(path, visible, material)` the way a render scene walks the
/// composed tree: each node with its ancestors, nearest first.
fn walk(
    node: &openbim_ifcx::ComposedNode,
    parent_visible: bool,
    ancestors: &mut Vec<NodePresentation>,
    out: &mut Vec<(String, bool, Material)>,
) {
    let own = NodePresentation::from_attributes(node).unwrap();
    let visible = is_visible(parent_visible, own.visibility);
    ancestors.insert(0, own);
    out.push((
        node.path.clone(),
        visible,
        resolve_mesh_material(ancestors.iter()),
    ));
    for child in node.children.values() {
        walk(child, visible, ancestors, out);
    }
    ancestors.remove(0);
}

#[test]
fn materials_bound_through_inherits_reach_composed_nodes() {
    // ifcx_alpha binds a material by inheriting from the material node.
    let nodes: Vec<openbim_ifcx::IfcxNode> = serde_json::from_value(json!([
        {"path": "glass", "attributes": {"bsi::ifc::presentation::diffuseColor": [0.5, 0.8, 0.6],
                                         "bsi::ifc::presentation::opacity": 0.3}},
        {"path": "window", "inherits": {"material": "glass"},
         "children": {"Body": "pane", "Frame": "frame"}},
        {"path": "pane", "attributes": {"usd::usdgeom::mesh": {}}},
        {"path": "frame", "attributes": {"usd::usdgeom::visibility": {"visibility": "invisible"}},
         "children": {"Body": "frame-body"}},
        {"path": "frame-body", "attributes": {"usd::usdgeom::mesh": {}}}
    ]))
    .unwrap();
    let composed = openbim_ifcx::compose(&openbim_ifcx::flatten(&nodes)).unwrap();
    let mut out = Vec::new();
    walk(
        composed.get("window").unwrap(),
        true,
        &mut Vec::new(),
        &mut out,
    );
    let glass = Material::Basic(BasicMaterial {
        color: [0.5, 0.8, 0.6],
        opacity: 0.3,
    });
    let find = |path: &str| out.iter().find(|(p, ..)| p == path).unwrap();
    assert!(find("window").1);
    assert_eq!(find("window").2, glass);
    assert_eq!(find("pane").2, glass, "child inherits the bound colour");
    assert!(find("pane").1);
    assert!(!find("frame").1);
    assert!(!find("frame-body").1, "hidden with its parent");
}

fn own(attributes: Value) -> Result<NodePresentation, DecodeError> {
    NodePresentation::from_attributes(attributes.as_object().unwrap())
}

#[test]
fn decodes_a_nodes_own_attributes() {
    let storey = presentation("storey");
    assert_eq!(storey.diffuse_color, Some([0.2, 0.4, 0.8]));
    assert_eq!(storey.opacity, Some(0.3));
    assert_eq!(storey.visibility, None);
    assert_eq!(storey.gltf_material, None);
    assert_eq!(
        presentation("opening").visibility,
        Some(Visibility::Invisible)
    );
    assert_eq!(
        presentation("frame").visibility,
        Some(Visibility::Inherited)
    );
    assert_eq!(own(json!({})).unwrap(), NodePresentation::default());
}

#[test]
fn gltf_factors_map_onto_the_material_type() {
    let wall = presentation("wall").gltf_material.unwrap();
    assert_eq!(
        wall,
        GltfMaterial {
            base_color_factor: [1.0, 0.9, 0.9, 1.0],
            base_color_texture: Some("textures/brick.webp".into()),
            metallic_factor: 0.2,
            roughness_factor: 0.7,
            metallic_roughness_texture: None,
            normal_texture: Some(NormalTexture {
                texture: "textures/brick-normal.webp".into(),
                scale: 0.5,
            }),
            occlusion_texture: Some(OcclusionTexture {
                texture: "textures/brick-ao.webp".into(),
                strength: 1.0,
            }),
            emissive_texture: None,
            emissive_factor: [0.5, 0.5, 1.0],
            alpha_mode: AlphaMode::Mask,
            alpha_cutoff: 0.25,
            double_sided: true,
        }
    );
}

#[test]
fn missing_gltf_members_take_gltf_defaults() {
    let p =
        own(json!({"gltf::material": {"pbrMetallicRoughness": {"metallicFactor": 0}}})).unwrap();
    let expected = GltfMaterial {
        metallic_factor: 0.0,
        ..GltfMaterial::default()
    };
    assert_eq!(p.gltf_material, Some(expected));
    assert_eq!(GltfMaterial::default().base_color_factor, [1.0; 4]);
    assert_eq!(GltfMaterial::default().roughness_factor, 1.0);
    assert_eq!(GltfMaterial::default().alpha_cutoff, 0.5);
    // An empty object, or one with only unknown members, is not a material.
    assert_eq!(
        own(json!({"gltf::material": {}})).unwrap().gltf_material,
        None
    );
    assert_eq!(
        own(json!({"gltf::material": {"extensions": {}}}))
            .unwrap()
            .gltf_material,
        None
    );
}

#[test]
fn invisible_hides_the_whole_subtree() {
    let storey = is_visible(true, presentation("storey").visibility);
    let opening = is_visible(storey, presentation("opening").visibility);
    let frame = is_visible(opening, presentation("frame").visibility);
    let wall = is_visible(storey, presentation("wall").visibility);
    assert!(storey && wall);
    assert!(!opening);
    assert!(!frame, "inherited cannot override a hidden ancestor");
    assert!(is_visible(true, Some(Visibility::Inherited)));
}

#[test]
fn nearest_gltf_material_wins_for_meshes() {
    let (storey, wall, body) = (
        presentation("storey"),
        presentation("wall"),
        presentation("body"),
    );
    // The body's own diffuseColor loses to the wall's glTF material.
    match resolve_mesh_material([&body, &wall, &storey]) {
        Material::Pbr(pbr) => assert_eq!(pbr.roughness_factor, 0.7),
        other => panic!("expected PBR, got {other:?}"),
    }
    // Curves ignore glTF and take the nearest diffuseColor.
    assert_eq!(
        resolve_basic_material([&body, &wall, &storey]),
        BasicMaterial {
            color: [1.0, 1.0, 1.0],
            opacity: 1.0
        }
    );
}

#[test]
fn colour_and_opacity_come_from_the_same_node() {
    let storey = presentation("storey");
    let tinted_child = own(json!({"bsi::ifc::presentation::opacity": 0.9})).unwrap();
    let material = resolve_basic_material([&tinted_child, &storey]);
    // The child's opacity is not used: it has no diffuseColor.
    assert_eq!(
        material,
        BasicMaterial {
            color: [0.2, 0.4, 0.8],
            opacity: 0.3
        }
    );
    assert!(material.is_transparent());
    assert_eq!(
        resolve_mesh_material([&tinted_child, &storey]),
        Material::Basic(material)
    );
}

#[test]
fn defaults_to_opaque_grey() {
    let bare = NodePresentation::default();
    assert_eq!(
        resolve_basic_material([&bare, &bare]),
        BasicMaterial::DEFAULT
    );
    assert_eq!(BasicMaterial::DEFAULT.color, [0.6; 3]);
    assert!(!BasicMaterial::DEFAULT.is_transparent());
    assert_eq!(
        resolve_mesh_material(std::iter::empty::<&NodePresentation>()),
        Material::Basic(BasicMaterial::DEFAULT)
    );
}

#[test]
fn zero_opacity_is_fully_transparent() {
    let p = own(json!({"bsi::ifc::presentation::diffuseColor": [1, 0, 0],
                       "bsi::ifc::presentation::opacity": 0}))
    .unwrap();
    assert_eq!(resolve_basic_material([&p]).opacity, 0.0);
}

fn wrong(at: &str, expected: &'static str) -> DecodeError {
    DecodeError::WrongShape {
        at: at.to_owned(),
        expected,
    }
}

#[test]
fn malformed_presentation_is_a_typed_error() {
    for (attributes, error) in [
        (
            json!({"usd::usdgeom::visibility": "invisible"}),
            wrong("usd::usdgeom::visibility", "object"),
        ),
        (
            json!({"usd::usdgeom::visibility": {}}),
            DecodeError::MissingField {
                field: "visibility",
            },
        ),
        (
            json!({"usd::usdgeom::visibility": {"visibility": "hidden"}}),
            DecodeError::UnknownToken {
                at: "visibility".into(),
                token: "hidden".into(),
                expected: "inherited or invisible",
            },
        ),
        (
            json!({"bsi::ifc::presentation::diffuseColor": [1, 0]}),
            wrong("bsi::ifc::presentation::diffuseColor", "array of 3 numbers"),
        ),
        (
            json!({"bsi::ifc::presentation::opacity": "0.5"}),
            wrong("bsi::ifc::presentation::opacity", "number"),
        ),
        (
            json!({"gltf::material": [1]}),
            wrong("gltf::material", "object"),
        ),
        (
            json!({"gltf::material": {"alphaMode": "CLEAR"}}),
            DecodeError::UnknownToken {
                at: "alphaMode".into(),
                token: "CLEAR".into(),
                expected: "OPAQUE, MASK, or BLEND",
            },
        ),
        (
            json!({"gltf::material": {"pbrMetallicRoughness": {"baseColorFactor": [1, 1, 1]}}}),
            wrong("pbrMetallicRoughness.baseColorFactor", "array of 4 numbers"),
        ),
        (
            json!({"gltf::material": {"pbrMetallicRoughness": {"metallicFactor": "high"}}}),
            wrong("pbrMetallicRoughness.metallicFactor", "number"),
        ),
        (
            json!({"gltf::material": {"normalTexture": {"scale": 1}}}),
            DecodeError::MissingField { field: "texture" },
        ),
        (
            json!({"gltf::material": {"occlusionTexture": {"texture": 7}}}),
            wrong("occlusionTexture.texture", "string"),
        ),
        (
            json!({"gltf::material": {"doubleSided": 1}}),
            wrong("doubleSided", "boolean"),
        ),
    ] {
        assert_eq!(own(attributes.clone()), Err(error), "{attributes}");
    }
}
