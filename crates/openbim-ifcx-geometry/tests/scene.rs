//! Render scenes of the hand-written fixtures under `tests/fixtures`.

use openbim_ifcx::{compose, flatten, Composition, IfcxFile};
use openbim_ifcx_geometry::presentation::DIFFUSE_COLOR;
use openbim_ifcx_geometry::scene::{
    GeometryKind, GeometryRef, Instance, Origin, RenderScene, SceneOptions, SceneWarningKind,
};
use openbim_ifcx_geometry::{
    curves, mesh, AlphaMode, BasicMaterial, DecodeError, GltfMaterial, Material, Transform, Vec3,
};

fn composition(name: &str) -> Composition {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    let file = IfcxFile::from_json_slice(&std::fs::read(path).unwrap()).unwrap();
    compose(&flatten(&file.data)).unwrap()
}

fn scene(name: &str) -> RenderScene {
    RenderScene::from_composition(&composition(name), &SceneOptions::default())
}

fn only<'s>(scene: &'s RenderScene, path: &str, kind: GeometryKind) -> &'s Instance {
    let mut found = scene
        .instances_at(path)
        .iter()
        .filter(|i| i.geometry.kind() == kind);
    let instance = found
        .next()
        .unwrap_or_else(|| panic!("no {kind:?} at {path}"));
    assert!(found.next().is_none());
    instance
}

/// Applies a column-major `f32` render matrix to a buffer position.
fn apply(m: &[f32; 16], p: [f32; 3]) -> Vec3 {
    let m = m.map(f64::from);
    let p = p.map(f64::from);
    [0, 1, 2].map(|r| m[r] * p[0] + m[4 + r] * p[1] + m[8 + r] * p[2] + m[12 + r])
}

fn close(a: Vec3, b: Vec3, tolerance: f64) -> bool {
    a.iter().zip(&b).all(|(x, y)| (x - y).abs() <= tolerance)
}

#[test]
fn placed_storey_walks_the_composed_hierarchy() {
    let scene = RenderScene::from_composition(
        &composition("placed-storey.ifcx"),
        &SceneOptions::default().with_origin(Origin::At([0.0; 3])),
    );
    let paths: Vec<_> = scene.instances.iter().map(|i| i.path.as_str()).collect();
    assert_eq!(paths, ["site/Storey/Wall", "site/Storey/Axis"]);
    assert!(scene.warnings.is_empty());

    let wall = only(&scene, "site/Storey/Wall", GeometryKind::Mesh);
    assert_eq!(wall.node, "wall");
    assert_eq!(
        wall.world.rows(),
        &[
            [0.0, 1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [100.0, 200.0, 3.0, 1.0],
        ],
        "a node without a transform inherits its parent's world matrix"
    );
    let axis = only(&scene, "site/Storey/Axis", GeometryKind::Lines);
    assert_eq!(axis.world.translation(), [100.0, 201.0, 3.0]);

    // The wall's vertices, through buffer and render matrix, land where the
    // f64 world transform puts them.
    let GeometryRef::Mesh(m) = wall.geometry else {
        unreachable!()
    };
    let placed: Vec<Vec3> = scene.meshes[m]
        .positions
        .iter()
        .map(|&p| apply(&wall.matrix, p))
        .collect();
    let expected = [
        [100.0, 200.0, 3.0],
        [100.0, 202.0, 3.0],
        [100.0, 200.0, 6.0],
    ];
    for (p, e) in placed.iter().zip(expected) {
        assert!(close(*p, e, 1e-4), "{p:?} != {e:?}");
    }
    let bounds = scene.bounds.unwrap();
    assert_eq!(bounds.min, [100.0, 200.0, 3.0]);
    assert_eq!(bounds.max, [100.0, 205.0, 6.0]);
}

#[test]
fn shared_types_are_stored_once_and_instanced() {
    let scene = scene("instanced-types.ifcx");
    let panels: Vec<_> = ["building/Left", "building/Right", "building/Mirror"]
        .map(|path| only(&scene, path, GeometryKind::Mesh))
        .into();
    assert!(panels.iter().all(|i| i.geometry == panels[0].geometry));
    let nodes: Vec<_> = panels.iter().map(|i| i.node.as_str()).collect();
    assert_eq!(nodes, ["left", "right", "mirror"]);
    // The panel and the wall body; the broken mesh and the hidden child are
    // not stored, and the hidden panel reuses the shared one.
    assert_eq!(scene.meshes.len(), 2);
    assert_eq!(scene.instances_at("building/Hidden").len(), 0);
    assert_eq!(scene.instances_at("building/Hidden/Child").len(), 0);

    let left = panels[0];
    let right = panels[1];
    assert_eq!(left.world.translation(), [-2.0, 0.0, 0.0]);
    assert_eq!(right.world.translation(), [2.0, 0.0, 0.0]);
    assert_eq!(left.bounds.min, [-2.0, 0.0, 0.0]);
    assert_eq!(right.bounds.max, [3.0, 0.0, 2.0]);
    assert!(!left.mirrored());
    assert!(panels[2].mirrored(), "negative determinant");
    assert_eq!(panels[2].bounds.min, [-1.0, 3.0, 0.0]);

    let GeometryRef::Mesh(m) = left.geometry else {
        unreachable!()
    };
    let panel = &scene.meshes[m];
    assert_eq!(panel.anchor, [0.5, 0.0, 1.0]);
    assert_eq!(panel.positions[0], [-0.5, 0.0, -1.0]);
    assert_eq!(panel.indices, [0, 1, 2, 0, 2, 3]);
    // Counter-clockwise seen from -Y: the face normal is -Y at every vertex.
    assert!(panel.normals.iter().all(|n| *n == [0.0, -1.0, 0.0]));
}

#[test]
fn materials_resolve_through_inherits_and_ancestors() {
    let scene = scene("instanced-types.ifcx");
    let material = |i: &Instance| &scene.materials[i.material];
    let glass = Material::Pbr(GltfMaterial {
        base_color_factor: [0.5, 0.7, 0.9, 0.4],
        metallic_factor: 0.0,
        roughness_factor: 0.1,
        alpha_mode: AlphaMode::Blend,
        ..GltfMaterial::default()
    });
    let brick = Material::Basic(BasicMaterial {
        color: [0.8, 0.3, 0.2],
        opacity: 1.0,
    });
    let left = only(&scene, "building/Left", GeometryKind::Mesh);
    assert_eq!(material(left), &glass);
    assert_eq!(
        left.material,
        only(&scene, "building/Mirror", GeometryKind::Mesh).material,
        "one table entry per distinct material"
    );
    // The wall binds brick; its body and axis inherit it from their parent.
    assert_eq!(
        material(only(&scene, "building/Wall/Body", GeometryKind::Mesh)),
        &brick
    );
    assert_eq!(
        material(only(&scene, "building/Wall/Axis", GeometryKind::Lines)),
        &brick
    );
    // Points carry their own colours; the material is white so they show.
    let scan = only(&scene, "building/Scan", GeometryKind::Points);
    assert_eq!(
        material(scan),
        &Material::Basic(BasicMaterial {
            color: [1.0; 3],
            opacity: 1.0
        })
    );
    let raw = only(&scene, "building/Scan/Raw", GeometryKind::Points);
    assert_eq!(
        material(raw),
        &Material::Basic(BasicMaterial {
            color: [0.0; 3],
            opacity: 1.0
        }),
        "points without colours are black"
    );
    let distinct: std::collections::HashSet<_> =
        scene.materials.iter().map(|m| format!("{m:?}")).collect();
    assert_eq!(distinct.len(), scene.materials.len());
}

#[test]
fn lines_and_points_fill_their_buffers() {
    let scene = scene("instanced-types.ifcx");
    let axis = only(&scene, "building/Wall/Axis", GeometryKind::Lines);
    let GeometryRef::Lines(l) = axis.geometry else {
        unreachable!()
    };
    let lines = &scene.lines[l];
    assert_eq!(lines.positions.len(), 5);
    // Two curves, both closed by `wrap: periodic`: a two-vertex curve gets
    // no extra segment, the triangle gets its closing one.
    assert_eq!(lines.indices, [0, 1, 2, 3, 3, 4, 4, 2]);
    assert_eq!(scene.segment_count(), 4);

    let GeometryRef::Points(p) = only(&scene, "building/Scan", GeometryKind::Points).geometry
    else {
        unreachable!()
    };
    let scan = &scene.points[p];
    assert_eq!(scan.positions.len(), 3);
    assert_eq!(
        scan.colors.as_deref(),
        Some(&[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]][..])
    );
    let raw = only(&scene, "building/Scan/Raw", GeometryKind::Points);
    assert_eq!(raw.bounds.min, [1.0, 2.0, 3.0]);
    assert_eq!(raw.bounds.max, [1.0, 2.0, 4.0]);
    // Scan (3) + raw (2) + the broken node's valid point cloud (2).
    assert_eq!(scene.point_count(), 7);
}

#[test]
fn decode_failures_become_warnings() {
    let scene = scene("instanced-types.ifcx");
    let mut warnings: Vec<_> = scene
        .warnings
        .iter()
        .map(|w| (w.path.as_str(), w.attribute, w.kind.clone()))
        .collect();
    warnings.sort_by_key(|w| w.1);
    assert_eq!(warnings.len(), 3, "{:?}", scene.warnings);
    assert_eq!(warnings[0].0, "building/Broken");
    assert_eq!(warnings[0].1, DIFFUSE_COLOR);
    assert_eq!(warnings[1].1, curves::ATTRIBUTE);
    assert!(matches!(
        &warnings[1].2,
        SceneWarningKind::UnsupportedCurve(c) if c.curve_type == "cubic"
    ));
    assert_eq!(warnings[2].1, mesh::ATTRIBUTE);
    assert!(matches!(
        warnings[2].2,
        SceneWarningKind::Decode(DecodeError::IndexOutOfRange { index: 3, .. })
    ));
    assert_eq!(
        scene.warnings.iter().find(|w| w.attribute == mesh::ATTRIBUTE).unwrap().to_string(),
        "building/Broken usd::usdgeom::mesh: faceVertexIndices[2]: index 3 out of range for 3 vertices"
    );
    // The rest of the broken node is still drawn, in the inherited colour.
    let broken = scene.instances_at("building/Broken");
    assert_eq!(broken.len(), 1);
    assert_eq!(broken[0].geometry.kind(), GeometryKind::Points);
}

#[test]
fn malformed_transform_drops_the_subtree() {
    let nodes: Vec<openbim_ifcx::IfcxNode> = serde_json::from_value(serde_json::json!([
        {"path": "a", "children": {"B": "b"},
         "attributes": {"usd::xformop": {"transform": [[1, 0, 0, 5], [0, 1, 0, 0], [0, 0, 1, 0], [0, 0, 0, 1]]}}},
        {"path": "b", "attributes": {"usd::usdgeom::basiscurves": {"points": [[0, 0, 0], [1, 0, 0]]}}},
        {"path": "c", "attributes": {"usd::usdgeom::basiscurves": {"points": [[0, 0, 0], [1, 0, 0]]},
                                     "usd::xformop": null}}
    ]))
    .unwrap();
    let scene = RenderScene::from_composition(
        &compose(&flatten(&nodes)).unwrap(),
        &SceneOptions::default(),
    );
    let paths: Vec<_> = scene.instances.iter().map(|i| i.path.as_str()).collect();
    assert_eq!(paths, ["c"], "null attributes count as absent");
    assert_eq!(scene.warnings.len(), 1);
    assert_eq!(scene.warnings[0].path, "a");
    assert_eq!(
        scene.lines.len(),
        1,
        "b and c write equal curves separately"
    );
}

#[test]
fn georeferenced_coordinates_keep_precision() {
    let composition = composition("georeferenced.ifcx");
    let scene = RenderScene::from_composition(&composition, &SceneOptions::default());
    let bounds = scene.bounds.unwrap();
    assert_eq!(bounds.min, [512000.001, 5612000.002, 100.0]);
    assert_eq!(bounds.max, [512010.004, 5612010.007, 102.25]);
    assert_eq!(scene.origin, bounds.center());

    // Every vertex, through the f32 buffer and matrix plus the f64 origin,
    // comes back within a tenth of a millimetre.
    let surface = only(&scene, "terrain/Surface", GeometryKind::Mesh);
    let GeometryRef::Mesh(m) = surface.geometry else {
        unreachable!()
    };
    assert_eq!(scene.meshes[m].anchor, [512005.0025, 5612005.0045, 101.875]);
    let expected = [
        [512000.001, 5612000.002, 101.5],
        [512010.004, 5612000.002, 101.5],
        [512010.004, 5612010.007, 102.25],
        [512000.001, 5612010.007, 102.25],
    ];
    for (p, e) in scene.meshes[m].positions.iter().zip(expected) {
        let rendered = apply(&surface.matrix, *p);
        let world = [0, 1, 2].map(|k| rendered[k] + scene.origin[k]);
        assert!(close(world, e, 1e-4), "{world:?} != {e:?}");
        assert!(rendered.iter().all(|c| c.abs() < 10.0), "small after shift");
    }
    let survey = only(&scene, "terrain/Survey", GeometryKind::Lines);
    assert_eq!(
        survey.world,
        Transform::from_translation([512000.0, 5612000.0, 100.0])
    );
    let GeometryRef::Lines(l) = survey.geometry else {
        unreachable!()
    };
    let end = apply(&survey.matrix, scene.lines[l].positions[1]);
    let end = [0, 1, 2].map(|k| end[k] + scene.origin[k]);
    assert!(close(end, [512010.004, 5612010.007, 100.0], 1e-4));

    // A fixed origin at zero leaves the large translation in the matrix.
    let unshifted = RenderScene::from_composition(
        &composition,
        &SceneOptions::default().with_origin(Origin::At([0.0; 3])),
    );
    assert_eq!(unshifted.origin, [0.0; 3]);
    let matrix = only(&unshifted, "terrain/Surface", GeometryKind::Mesh).matrix;
    assert_eq!(matrix[12], 512005.0025f64 as f32);
}

#[test]
fn empty_composition_gives_an_empty_scene() {
    let scene =
        RenderScene::from_composition(&compose(&flatten(&[])).unwrap(), &SceneOptions::default());
    assert!(scene.instances.is_empty());
    assert_eq!(scene.bounds, None);
    assert_eq!(scene.origin, [0.0; 3]);
}
