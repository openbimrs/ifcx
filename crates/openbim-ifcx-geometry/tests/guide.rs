//! The code of the documentation site's Rust guide (`docs/guide/rust.md`).
//!
//! The guide imports the `#region` blocks below verbatim, so every snippet
//! it shows compiles and runs in the gate. Edit the code here, not on the
//! page.

use std::error::Error;
use std::path::PathBuf;

fn fixture(crate_dir: &str, name: &str) -> String {
    let path: PathBuf = [
        env!("CARGO_MANIFEST_DIR"),
        "..",
        crate_dir,
        "tests/fixtures",
        name,
    ]
    .iter()
    .collect();
    path.to_string_lossy().into_owned()
}

#[test]
fn read_and_write() -> Result<(), Box<dyn Error>> {
    let path = fixture("openbim-ifcx", "geometry-model.ifcx");
    // #region read
    use openbim_ifcx::IfcxFile;

    let bytes = std::fs::read(&path)?;
    let file = IfcxFile::from_json_slice(&bytes)?;
    println!("{}: {} nodes", file.header.id, file.data.len());

    // Writing back is lossless: key order, unknown fields, `null`
    // deletions and every number stay as read.
    let text = file.to_json_string_pretty()?;
    assert_eq!(IfcxFile::from_json_str(&text)?, file);
    // #endregion read
    Ok(())
}

#[test]
fn validate() -> Result<(), Box<dyn Error>> {
    let path = fixture("openbim-ifcx", "invalid-attributes.ifcx");
    use openbim_ifcx::IfcxFile;
    let file = IfcxFile::from_json_slice(&std::fs::read(&path)?)?;
    // #region validate
    // Check every attribute against the file's own `schemas`.
    if let Err(report) = file.validate() {
        for failure in &report.failures {
            // failure.node, .attribute, .pointer (JSON pointer), .kind
            println!("{failure}");
        }
    }
    // #endregion validate
    assert!(file.validate().is_err());
    Ok(())
}

#[test]
fn compose_layers() -> Result<(), Box<dyn Error>> {
    let path = fixture("openbim-ifcx", "geometry-model.ifcx");
    use openbim_ifcx::IfcxFile;
    let file = IfcxFile::from_json_slice(&std::fs::read(&path)?)?;
    // #region compose
    use openbim_ifcx::{compose, flatten_owned};

    // Flatten the nodes by path (later opinions win), then compose them
    // into a tree: `inherits` expanded, children resolved.
    let composition = compose(&flatten_owned(file.data))?;
    for root in composition.roots() {
        let node = composition.get(root).expect("roots are in the tree");
        println!("{root}: {} children", node.children.len());
    }
    // #endregion compose
    assert!(!composition.roots().is_empty());
    Ok(())
}

#[test]
fn imports_from_disk() -> Result<(), Box<dyn Error>> {
    let main = fixture("openbim-ifcx", "layers/chain/main.ifcx");
    // #region imports
    use openbim_ifcx::layers::{FsResolver, LayerStackBuilder};

    // Load the file with every file its `imports` name, recursively.
    // `FsResolver` (feature `fs`) reads local files; anything else, such as
    // an HTTP client, implements `LayerResolver`.
    let stack = LayerStackBuilder::new(FsResolver::new()).build(&main)?;
    stack.validate()?; // against the schemas of every layer
    let federated = stack.into_federated(); // one file, ready to compose
                                            // #endregion imports
    assert!(!federated.data.is_empty());
    Ok(())
}

#[test]
fn export_glb() -> Result<(), Box<dyn Error>> {
    let path = fixture("openbim-ifcx", "geometry-model.ifcx");
    let out = std::env::temp_dir().join(format!("ifcx-guide-{}.glb", std::process::id()));
    // #region glb
    use openbim_ifcx::{compose, flatten_owned, IfcxFile};
    use openbim_ifcx_geometry::{to_glb, GlbOptions, RenderScene, SceneOptions};

    let file = IfcxFile::from_json_slice(&std::fs::read(&path)?)?;
    let composition = compose(&flatten_owned(file.data))?;

    // The flat scene a viewer draws: instances with their IFCX path and
    // world matrix, shared f32 buffers, materials, bounds.
    let scene = RenderScene::from_composition(&composition, &SceneOptions::default());
    println!("{} instances", scene.instances.len());

    // Binary glTF 2.0, Y-up, one node per instance named by its IFCX path.
    let glb = to_glb(&scene, &GlbOptions::default())?;
    std::fs::write(&out, glb)?;
    // #endregion glb
    assert!(std::fs::metadata(&out)?.len() > 0);
    std::fs::remove_file(&out)?;
    Ok(())
}
