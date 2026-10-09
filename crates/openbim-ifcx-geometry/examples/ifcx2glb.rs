//! Converts one or more IFCX files into a binary glTF (`.glb`) file that any
//! glTF viewer opens.
//!
//! ```sh
//! cargo run --release -p openbim-ifcx-geometry --example ifcx2glb -- model.ifcx model.glb
//! cargo run --release -p openbim-ifcx-geometry --example ifcx2glb -- base.ifcx overlay.ifcx out.glb
//! cargo run --release -p openbim-ifcx-geometry --example ifcx2glb -- \
//!     --resolve-imports --mirror https://ifcx.dev/=mirror/ifcx.dev model.ifcx model.glb
//! ```
//!
//! The last argument is the output file; every other argument is a layer, in
//! order, weakest first: their `data` arrays are concatenated, flattened, and
//! composed, so the last layer wins.
//!
//! Options, before the files:
//!
//! - `--resolve-imports`: load the `imports` of every layer, recursively,
//!   through `openbim_ifcx::layers::FsResolver`, as upstream's `ifcx compose`
//!   does: the layers become the imports of a main layer without data, so
//!   each layer overrides its imports and the next layer overrides both. A relative import resolves against the importing file's
//!   directory. Without this option imports are ignored; `ifcx_alpha`
//!   examples import only schema files, which carry no `data`.
//! - `--mirror PREFIX=DIR` (repeatable, implies `--resolve-imports`): load
//!   imports whose URI starts with `PREFIX` from the rest of the URI below
//!   `DIR`, for example `https://ifcx.dev/=mirror/ifcx.dev` for an offline
//!   copy of `ifcx.dev`. The first matching prefix applies. Nothing is
//!   fetched from the network; an import with a scheme and no mirror fails.
//! - `--z-up`: keep IFCX's Z-up axes instead of rotating to glTF's Y-up.
//! - `--local`: leave the scene origin off the root node, so the model sits
//!   around the glTF origin (the origin is still in the root's `extras`).
//!
//! Prints a summary and any warnings to stderr. Exits with code 1 if a file
//! or an import cannot be read, an `integrity` value does not match, the
//! layers do not compose, or the output cannot be written.

use std::process::ExitCode;

use openbim_ifcx::layers::{FsResolver, LayerStackBuilder};
use openbim_ifcx::{compose, flatten_owned, IfcxFile};
use openbim_ifcx_geometry::glb::{to_glb, GlbOptions};
use openbim_ifcx_geometry::scene::{RenderScene, SceneOptions};

const USAGE: &str = "usage: ifcx2glb [--resolve-imports] [--mirror PREFIX=DIR]... [--z-up] [--local] LAYER.ifcx... OUT.glb";

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
    let mut resolver: Option<FsResolver> = None;
    let mut paths = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--z-up" => options = options.with_y_up(false),
            "--local" => options = options.with_origin_on_root(false),
            "--resolve-imports" => {
                resolver.get_or_insert_with(FsResolver::new);
            }
            "--mirror" => {
                let mapping = args.next().ok_or("--mirror needs PREFIX=DIR")?;
                let (prefix, dir) = mapping
                    .split_once('=')
                    .filter(|(prefix, dir)| !prefix.is_empty() && !dir.is_empty())
                    .ok_or_else(|| format!("--mirror {mapping:?}: expected PREFIX=DIR"))?;
                resolver = Some(resolver.unwrap_or_default().map_prefix(prefix, dir));
            }
            "-h" | "--help" => {
                eprintln!("{USAGE}");
                return Ok(());
            }
            _ => paths.push(arg),
        }
    }
    let Some((out, layers)) = paths.split_last().filter(|(_, layers)| !layers.is_empty()) else {
        return Err(USAGE.into());
    };

    let (data, layer_count) = match resolver {
        Some(resolver) => {
            let stack = LayerStackBuilder::new(resolver)
                .build_all(layers)
                .map_err(|e| e.to_string())?;
            let count = stack.layers().len();
            (stack.into_federated().data, count)
        }
        None => {
            let mut data = Vec::new();
            for path in layers {
                let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
                let file = IfcxFile::from_json_slice(&bytes).map_err(|e| format!("{path}: {e}"))?;
                data.extend(file.data);
            }
            (data, layers.len())
        }
    };
    let composition = compose(&flatten_owned(data)).map_err(|e| e.to_string())?;
    let scene = RenderScene::from_composition(&composition, &SceneOptions::default());
    for warning in &scene.warnings {
        eprintln!("warning: {warning}");
    }
    let glb = to_glb(&scene, &options).map_err(|e| e.to_string())?;
    std::fs::write(out, &glb).map_err(|e| format!("{out}: {e}"))?;
    eprintln!(
        "{out}: {layer_count} layers, {} instances, {} triangles, {} segments, {} points, {} warnings, {} bytes; origin {:?}",
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
