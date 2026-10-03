//! Geometry that needs an import: a layer whose instances inherit a type
//! defined only in the file it imports, resolved from disk as
//! `ifcx2glb --resolve-imports` does.

use std::path::PathBuf;

use openbim_ifcx::layers::{FsResolver, LayerStackBuilder};
use openbim_ifcx::{compose, flatten_owned, ComposeError, IfcxFile};
use openbim_ifcx_geometry::glb::{to_glb, GlbOptions};
use openbim_ifcx_geometry::scene::{RenderScene, SceneOptions};
use serde_json::Value;

fn fixture(name: &str) -> String {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", name]
        .iter()
        .collect();
    path.into_os_string().into_string().unwrap()
}

/// The JSON chunk of a GLB file.
fn glb_json(glb: &[u8]) -> Value {
    assert_eq!(&glb[..4], b"glTF");
    let len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    assert_eq!(&glb[16..20], b"JSON");
    serde_json::from_slice(&glb[20..20 + len]).unwrap()
}

#[test]
fn the_layer_alone_does_not_compose() {
    let file =
        IfcxFile::from_json_slice(&std::fs::read(fixture("imports-panel-type.ifcx")).unwrap())
            .unwrap();
    assert!(matches!(
        compose(&flatten_owned(file.data)),
        Err(ComposeError::UnknownReference { reference, .. }) if reference == "panel"
    ));
}

#[test]
fn with_its_import_resolved_the_layer_exports_its_panels() {
    let stack = LayerStackBuilder::new(FsResolver::new())
        .build_all([fixture("imports-panel-type.ifcx")])
        .unwrap();
    assert_eq!(stack.layers().len(), 2);
    let composition = compose(&flatten_owned(stack.into_federated().data)).unwrap();
    let scene = RenderScene::from_composition(&composition, &SceneOptions::default());
    let row: Vec<_> = scene
        .instances
        .iter()
        .filter(|instance| instance.path.starts_with("row/"))
        .collect();
    assert_eq!(row.len(), 2, "{:?}", scene.instances);
    // Both panels share the imported type's mesh and material.
    assert_eq!(row[0].geometry, row[1].geometry);
    assert_eq!(row[0].material, row[1].material);

    let json = glb_json(&to_glb(&scene, &GlbOptions::default()).unwrap());
    let names: Vec<&str> = json["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|node| node["name"].as_str())
        .collect();
    assert!(names.contains(&"row/A"), "{names:?}");
    assert!(names.contains(&"row/B"), "{names:?}");
}
