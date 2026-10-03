//! Decodes the hand-written fixtures under `tests/fixtures`.
//!
//! Composition is not implemented yet, so the walk below follows `children`
//! within a single file where every path has exactly one node. It stands in
//! for the composed-tree walk of the render scene.

use std::collections::HashMap;

use openbim_ifcx::{IfcxFile, IfcxNode};
use openbim_ifcx_geometry::{
    curves, mesh, transform, world_from_parent, CurveGeometry, Transform, TriangleMesh, Vec3,
};

fn load(name: &str) -> IfcxFile {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    IfcxFile::from_json_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn walk(
    nodes: &HashMap<&str, &IfcxNode>,
    path: &str,
    parent: &Transform,
    out: &mut Vec<(String, Transform)>,
) {
    let node = nodes[path];
    let local = node
        .attributes
        .as_ref()
        .and_then(|a| a.get(transform::ATTRIBUTE))
        .map(|v| Transform::from_attribute(v).unwrap());
    let world = world_from_parent(parent, local.as_ref());
    out.push((path.to_owned(), world));
    for child in node
        .children
        .iter()
        .flatten()
        .filter_map(|(_, c)| c.as_ref())
    {
        walk(nodes, child, &world, out);
    }
}

fn close(a: Vec3, b: Vec3) -> bool {
    a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1e-9)
}

#[test]
fn placed_storey_world_geometry() {
    let file = load("placed-storey.ifcx");
    let nodes: HashMap<&str, &IfcxNode> = file.data.iter().map(|n| (n.path.as_str(), n)).collect();
    let mut worlds = Vec::new();
    walk(&nodes, "site", &Transform::IDENTITY, &mut worlds);
    let world: HashMap<_, _> = worlds.into_iter().collect();

    assert_eq!(world["site"].translation(), [100.0, 200.0, 0.0]);
    assert_eq!(world["storey"].translation(), [100.0, 200.0, 3.0]);
    assert_eq!(world["wall"], world["storey"], "no transform inherits");
    assert_eq!(
        world["storey"].rows(),
        &[
            [0.0, 1.0, 0.0, 0.0],
            [-1.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [100.0, 200.0, 3.0, 1.0],
        ]
    );
    // Axis origin (1, 0, 0) is rotated onto +Y by the storey.
    assert_eq!(world["axis"].translation(), [100.0, 201.0, 3.0]);

    let wall =
        TriangleMesh::from_attribute(&nodes["wall"].attributes.as_ref().unwrap()[mesh::ATTRIBUTE])
            .unwrap();
    let placed: Vec<Vec3> = wall
        .positions
        .iter()
        .map(|&p| world["wall"].transform_point(p))
        .collect();
    let expected = [
        [100.0, 200.0, 3.0],
        [100.0, 202.0, 3.0],
        [100.0, 200.0, 6.0],
    ];
    for (p, e) in placed.iter().zip(&expected) {
        assert!(close(*p, *e), "{p:?} != {e:?}");
    }

    let CurveGeometry::Polylines(lines) = CurveGeometry::from_attribute(
        &nodes["axis"].attributes.as_ref().unwrap()[curves::ATTRIBUTE],
    )
    .unwrap() else {
        panic!("axis is linear")
    };
    let end = world["axis"].transform_point(lines[0].points[1]);
    assert!(close(end, [100.0, 205.0, 3.0]), "{end:?}");
}
