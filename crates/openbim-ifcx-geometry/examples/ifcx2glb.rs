//! Converts one or more IFCX files into a binary glTF (`.glb`) file that any
//! glTF viewer opens.
//!
//! ```sh
//! cargo run --release -p openbim-ifcx-geometry --example ifcx2glb -- model.ifcx model.glb
//! cargo run --release -p openbim-ifcx-geometry --example ifcx2glb -- base.ifcx overlay.ifcx out.glb
//! ```
//!
//! The last argument is the output file; every other argument is a layer, in
//! order, weakest first: their `data` arrays are concatenated, flattened, and
//! composed. Imports are not resolved; `ifcx_alpha` examples import only
//! schema files, which carry no `data`.
//!
//! Options, before the files:
//!
//! - `--z-up`: keep IFCX's Z-up axes instead of rotating to glTF's Y-up.
//! - `--local`: leave the scene origin off the root node, so the model sits
//!   around the glTF origin (the origin is still in the root's `extras`).
//!
//! Prints a summary and any warnings to stderr. Exits with code 1 if a file
//! cannot be read, the layers do not compose, or the output cannot be
//! written.

use std::process::ExitCode;

use openbim_ifcx::{compose, flatten, IfcxFile};
use openbim_ifcx_geometry::glb::{to_glb, GlbOptions};
use openbim_ifcx_geometry::scene::{RenderScene, SceneOptions};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("ifcx2glb: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let mut options = GlbOptions::default();
    let mut paths = Vec::new();
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--z-up" => options = options.with_y_up(false),
            "--local" => options = options.with_origin_on_root(false),
            "-h" | "--help" => {
                eprintln!("usage: ifcx2glb [--z-up] [--local] LAYER.ifcx... OUT.glb");
                return Ok(());
            }
            _ => paths.push(arg),
        }
    }
    let Some((out, layers)) = paths.split_last().filter(|(_, layers)| !layers.is_empty()) else {
        return Err("usage: ifcx2glb [--z-up] [--local] LAYER.ifcx... OUT.glb".into());
    };

    let mut data = Vec::new();
    for path in layers {
        let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        let file = IfcxFile::from_json_slice(&bytes).map_err(|e| format!("{path}: {e}"))?;
        data.extend(file.data);
    }
    let composition = compose(&flatten(&data)).map_err(|e| e.to_string())?;
    let scene = RenderScene::from_composition(&composition, &SceneOptions::default());
    for warning in &scene.warnings {
        eprintln!("warning: {warning}");
    }
    let glb = to_glb(&scene, &options).map_err(|e| e.to_string())?;
    std::fs::write(out, &glb).map_err(|e| format!("{out}: {e}"))?;
    eprintln!(
        "{out}: {} instances, {} triangles, {} segments, {} points, {} warnings, {} bytes; origin {:?}",
        scene.instances.len(),
        scene.triangle_count(),
        scene.segment_count(),
        scene.point_count(),
        scene.warnings.len(),
        glb.len(),
        scene.origin,
    );
    Ok(())
}
