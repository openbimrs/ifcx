//! Opt-in check against a local checkout of buildingSMART/IFC5-development.
//!
//! Decodes every `usd::xformop`, `usd::usdgeom::mesh`,
//! `usd::usdgeom::basiscurves`, point-cloud, and presentation attribute in
//! upstream's example files and reports counts and failures. Upstream
//! publishes no license, so its files are never committed here. Set
//! `IFCX_UPSTREAM_DIR` to a checkout to run this; without it the test passes
//! without doing anything.
//!
//! ```sh
//! IFCX_UPSTREAM_DIR=../IFC5-development cargo test --release -p openbim-ifcx-geometry \
//!     --test upstream_decode -- --nocapture
//! ```

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use openbim_ifcx::IfcxFile;
use openbim_ifcx_geometry::points::{PCD_BASE64, POINTS_ARRAY, POINTS_BASE64};
use openbim_ifcx_geometry::presentation::{DIFFUSE_COLOR, GLTF_MATERIAL, OPACITY, VISIBILITY};
use openbim_ifcx_geometry::{
    curves, mesh, transform, Attributes, CurveGeometry, NodePresentation, PointCloud, Transform,
    TriangleMesh,
};

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

#[derive(Default)]
struct Counts {
    transforms: usize,
    meshes: usize,
    triangles: usize,
    vertices: usize,
    curve_attributes: usize,
    polylines: usize,
    polyline_vertices: usize,
    unsupported_curves: usize,
    deletions: usize,
}

#[test]
fn upstream_geometry_attributes_decode() {
    let Some(dir) = std::env::var_os("IFCX_UPSTREAM_DIR") else {
        eprintln!("IFCX_UPSTREAM_DIR not set; skipping");
        return;
    };
    let mut files = Vec::new();
    ifcx_files(Path::new(&dir), &mut files);
    files.sort();
    assert!(!files.is_empty(), "no .ifcx files under {dir:?}");

    let mut total = Counts::default();
    let mut failures = Vec::new();
    for path in &files {
        let rel = path
            .strip_prefix(&dir)
            .unwrap_or(path)
            .display()
            .to_string();
        let file = IfcxFile::from_json_slice(&std::fs::read(path).unwrap())
            .unwrap_or_else(|e| panic!("{rel}: {e}"));
        let mut c = Counts::default();
        for node in &file.data {
            let Some(attributes) = &node.attributes else {
                continue;
            };
            for (name, value) in attributes {
                let known = [transform::ATTRIBUTE, mesh::ATTRIBUTE, curves::ATTRIBUTE];
                if !known.contains(&name.as_str()) {
                    continue;
                }
                if value.is_null() {
                    // A layer deleting the attribute; nothing to decode.
                    c.deletions += 1;
                    continue;
                }
                let result = match name.as_str() {
                    transform::ATTRIBUTE => Transform::from_attribute(value).map(|_| {
                        c.transforms += 1;
                    }),
                    mesh::ATTRIBUTE => TriangleMesh::from_attribute(value).map(|m| {
                        c.meshes += 1;
                        c.triangles += m.triangle_count();
                        c.vertices += m.positions.len();
                    }),
                    _ => CurveGeometry::from_attribute(value).map(|g| {
                        c.curve_attributes += 1;
                        match g {
                            CurveGeometry::Polylines(lines) => {
                                c.polylines += lines.len();
                                c.polyline_vertices +=
                                    lines.iter().map(|l| l.points.len()).sum::<usize>();
                            }
                            CurveGeometry::Unsupported(_) => c.unsupported_curves += 1,
                        }
                    }),
                };
                if let Err(e) = result {
                    failures.push(format!("{rel}: {} {name}: {e}", node.path));
                }
            }
        }
        eprintln!(
            "{:>6} xform {:>5} mesh {:>8} tri {:>4} curve  {rel}",
            c.transforms, c.meshes, c.triangles, c.curve_attributes
        );
        total.transforms += c.transforms;
        total.meshes += c.meshes;
        total.triangles += c.triangles;
        total.vertices += c.vertices;
        total.curve_attributes += c.curve_attributes;
        total.polylines += c.polylines;
        total.polyline_vertices += c.polyline_vertices;
        total.unsupported_curves += c.unsupported_curves;
        total.deletions += c.deletions;
    }
    eprintln!(
        "{} files: {} transforms; {} meshes ({} triangles, {} vertices); \
         {} curve attributes ({} polylines, {} vertices, {} unsupported); \
         {} null deletions; {} failures",
        files.len(),
        total.transforms,
        total.meshes,
        total.triangles,
        total.vertices,
        total.curve_attributes,
        total.polylines,
        total.polyline_vertices,
        total.unsupported_curves,
        total.deletions,
        failures.len()
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[derive(Default)]
struct PresentationCounts {
    clouds: [usize; 3],
    points: usize,
    colored: usize,
    presentation: [usize; 4],
    pbr: usize,
}

#[test]
fn upstream_point_clouds_and_presentation_decode() {
    let Some(dir) = std::env::var_os("IFCX_UPSTREAM_DIR") else {
        eprintln!("IFCX_UPSTREAM_DIR not set; skipping");
        return;
    };
    let mut files = Vec::new();
    ifcx_files(Path::new(&dir), &mut files);
    files.sort();
    assert!(!files.is_empty(), "no .ifcx files under {dir:?}");

    let mut counts = PresentationCounts::default();
    let mut failures = Vec::new();
    let mut total = Duration::ZERO;
    for path in &files {
        let name = path
            .strip_prefix(&dir)
            .unwrap_or(path)
            .display()
            .to_string();
        let file = IfcxFile::from_json_slice(&std::fs::read(path).unwrap()).unwrap();
        let start = Instant::now();
        let mut file_points = 0;
        for node in &file.data {
            // Each encoding on its own, not only the one the viewer prefers.
            for (i, (id, decode)) in [
                (PCD_BASE64, PointCloud::from_pcd_base64 as fn(_) -> _),
                (POINTS_ARRAY, PointCloud::from_points_array),
                (POINTS_BASE64, PointCloud::from_points_base64),
            ]
            .into_iter()
            .enumerate()
            {
                let Some(value) = node.attribute(id) else {
                    continue;
                };
                match decode(value) {
                    Ok(cloud) => {
                        assert!(cloud.positions.iter().flatten().all(|v| v.is_finite()));
                        counts.clouds[i] += 1;
                        counts.points += cloud.len();
                        counts.colored += usize::from(cloud.colors.is_some());
                        file_points += cloud.len();
                    }
                    Err(e) => failures.push(format!("{name} {}: {e}", node.path)),
                }
            }
            match NodePresentation::from_attributes(node) {
                Ok(p) => counts.pbr += usize::from(p.gltf_material.is_some()),
                Err(e) => failures.push(format!("{name} {}: {e}", node.path)),
            }
            for (i, id) in [VISIBILITY, DIFFUSE_COLOR, OPACITY, GLTF_MATERIAL]
                .into_iter()
                .enumerate()
            {
                counts.presentation[i] += usize::from(node.attribute(id).is_some());
            }
        }
        let elapsed = start.elapsed();
        total += elapsed;
        if file_points > 0 {
            eprintln!("{file_points:>8} points  {elapsed:>9.1?}  {name}");
        }
    }
    let [pcd, array, base64] = counts.clouds;
    let [visibility, color, opacity, gltf] = counts.presentation;
    eprintln!(
        "{} files in {total:.1?}: point clouds pcd {pcd}, array {array}, base64 {base64} \
         ({} points, {} coloured); visibility {visibility}, diffuseColor {color}, \
         opacity {opacity}, gltf::material {gltf} ({} PBR); {} failures",
        files.len(),
        counts.points,
        counts.colored,
        counts.pbr,
        failures.len()
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
