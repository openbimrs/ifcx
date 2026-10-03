//! Opt-in render scenes of every example in a local checkout of
//! buildingSMART/IFC5-development.
//!
//! Upstream publishes no license, so its files are never committed here. Set
//! `IFCX_UPSTREAM_DIR` to a checkout to run this; without it the test passes
//! without doing anything.
//!
//! ```sh
//! IFCX_UPSTREAM_DIR=../IFC5-development cargo test --release -p openbim-ifcx-geometry \
//!     --test upstream_scene -- --nocapture
//! ```
//!
//! Files are composed as in `openbim-ifcx`'s `upstream_composition` test:
//! alone, or, if a reference is unknown, on top of every other file of their
//! folder below `examples/`. Imports carry only schemas there, so they are
//! not loaded. For each file the test prints the build time, instance,
//! buffer, triangle, segment, and point counts, the origin, and warnings.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use openbim_ifcx::{compose, flatten, ComposeError, Composition, IfcxFile};
use openbim_ifcx_geometry::scene::{GeometryKind, RenderScene, SceneOptions};

fn ifcx_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            if path
                .file_name()
                .is_some_and(|n| n != ".git" && n != "node_modules")
            {
                ifcx_files(&path, out);
            }
        } else if path.extension().is_some_and(|e| e == "ifcx") {
            out.push(path);
        }
    }
}

fn example_folder(path: &Path) -> &Path {
    path.ancestors()
        .find(|dir| dir.parent().and_then(Path::file_name) == Some("examples".as_ref()))
        .unwrap_or_else(|| path.parent().unwrap())
}

fn compose_files(files: &[PathBuf]) -> Result<Composition, ComposeError> {
    let data: Vec<_> = files
        .iter()
        .flat_map(|path| {
            IfcxFile::from_json_slice(&std::fs::read(path).unwrap())
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
                .data
        })
        .collect();
    compose(&flatten(&data))
}

#[test]
fn upstream_examples_build_scenes() {
    let Some(dir) = std::env::var_os("IFCX_UPSTREAM_DIR") else {
        eprintln!("IFCX_UPSTREAM_DIR not set; skipping");
        return;
    };
    let dir = PathBuf::from(dir);
    let mut files = Vec::new();
    ifcx_files(&dir, &mut files);
    files.sort();
    assert!(!files.is_empty(), "no .ifcx files under {dir:?}");

    let mut total_time = Duration::ZERO;
    let (mut instances, mut triangles, mut segments, mut points, mut warnings) = (0, 0, 0, 0, 0);
    for path in &files {
        let name = path
            .strip_prefix(&dir)
            .unwrap_or(path)
            .display()
            .to_string();
        let mut composed = compose_files(std::slice::from_ref(path));
        if let Err(ComposeError::UnknownReference { .. }) = composed {
            let mut stack = Vec::new();
            ifcx_files(example_folder(path), &mut stack);
            stack.sort();
            stack.retain(|other| other != path);
            stack.push(path.clone());
            composed = compose_files(&stack);
        }
        let composed = composed.unwrap_or_else(|e| panic!("{name}: {e}"));

        let start = Instant::now();
        let scene = RenderScene::from_composition(&composed, &SceneOptions::default());
        let time = start.elapsed();
        total_time += time;

        let count = |kind| {
            scene
                .instances
                .iter()
                .filter(|i| i.geometry.kind() == kind)
                .count()
        };
        for instance in &scene.instances {
            assert!(
                composed.get(&instance.path).is_some(),
                "{name}: instance path {} does not resolve",
                instance.path
            );
            assert!(instance.matrix.iter().all(|v| v.is_finite()));
        }
        for warning in &scene.warnings {
            eprintln!("    warning: {warning}");
        }
        eprintln!(
            "{time:>10.1?} {:>6} inst ({:>5} mesh {:>3} line {:>2} pts) {:>5}/{:>3}/{:>2} buf {:>8} tri {:>6} seg {:>7} pts  origin {:?}  {name}",
            scene.instances.len(),
            count(GeometryKind::Mesh),
            count(GeometryKind::Lines),
            count(GeometryKind::Points),
            scene.meshes.len(),
            scene.lines.len(),
            scene.points.len(),
            scene.triangle_count(),
            scene.segment_count(),
            scene.point_count(),
            scene.origin.map(|c| (c * 1000.0).round() / 1000.0),
        );
        instances += scene.instances.len();
        triangles += scene.triangle_count();
        segments += scene.segment_count();
        points += scene.point_count();
        warnings += scene.warnings.len();
    }
    eprintln!(
        "{} files in {total_time:.1?}: {instances} instances, {triangles} triangles, \
         {segments} segments, {points} points, {warnings} warnings",
        files.len()
    );
    assert_eq!(warnings, 0);
}
