//! Composition of hand-written layers. Expected values were checked against
//! upstream's TypeScript implementation (buildingSMART/IFC5-development
//! `src/ifcx-core/composition`) run on the same fixtures.

use std::path::PathBuf;
use std::sync::Arc;

use indexmap::IndexMap;
use openbim_ifcx::{
    compose, flatten, ComposeError, ComposedNode, Composition, FlatNode, IfcxFile, IfcxNode,
};
use serde_json::{json, Map, Value};

fn fixture(name: &str) -> String {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", name]
        .iter()
        .collect();
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

fn read(name: &str) -> IfcxFile {
    IfcxFile::from_json_str(&fixture(name)).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// The data of `names` in layer order, weakest first.
fn layers(names: &[&str]) -> Vec<IfcxNode> {
    names.iter().flat_map(|name| read(name).data).collect()
}

/// Flattened nodes as JSON, in the shape upstream's `PreCompositionNode`
/// map serialises to.
fn flat_json(flat: &IndexMap<String, FlatNode>) -> Value {
    let mut out = Map::new();
    for (path, node) in flat {
        let attributes: Map<String, Value> = node
            .attributes
            .iter()
            .map(|(k, v)| (k.clone(), (**v).clone()))
            .collect();
        out.insert(
            path.clone(),
            json!({
                "children": node.children,
                "inherits": node.inherits,
                "attributes": attributes,
            }),
        );
    }
    Value::Object(out)
}

/// Every object's keys, depth first, so key order is compared too.
fn key_order(value: &Value, out: &mut Vec<String>) {
    if let Value::Object(map) = value {
        for (key, child) in map {
            out.push(key.clone());
            key_order(child, out);
        }
    }
}

#[test]
fn flattening_two_layers_matches_upstream() {
    let nodes = layers(&["layer-base.ifcx", "layer-edit.ifcx"]);
    let flat = flatten(&nodes);
    let expected = json!({
        "site": {
            "children": {"Wall": "wall", "Slab": null, "Roof": "roof"},
            "inherits": {},
            "attributes": {"example::name": "Site", "example::phase": "design"}
        },
        "wall": {
            "children": {"Body": "wall-body-v2", "Opening": null},
            "inherits": {"Type": "wall-type", "Finish": "finish-plaster"},
            "attributes": {
                "example::height": 3.0,
                "example::fireRated": null,
                "example::loadBearing": false
            }
        },
        "slab": {"children": {}, "inherits": {}, "attributes": {"example::thickness": 0.25}},
        "wall/Body": {"children": {}, "inherits": {}, "attributes": {"example::colour": "grey"}},
        "roof": {"children": {}, "inherits": {}, "attributes": {"example::pitch": 30}}
    });
    let actual = flat_json(&flat);
    assert_eq!(actual, expected);
    let (mut a, mut e) = (Vec::new(), Vec::new());
    key_order(&actual, &mut a);
    key_order(&expected, &mut e);
    assert_eq!(a, e);
}

#[test]
fn layer_order_decides_the_winner() {
    // The same two layers the other way round: the base now wins.
    let nodes = layers(&["layer-edit.ifcx", "layer-base.ifcx"]);
    let flat = flatten(&nodes);
    let wall = &flat["wall"];
    assert_eq!(wall.children["Body"].as_deref(), Some("wall-body"));
    assert_eq!(wall.children["Opening"].as_deref(), Some("opening"));
    assert_eq!(*wall.attributes["example::height"], json!(2.75));
    assert_eq!(*wall.attributes["example::fireRated"], json!(true));
    // A null inherit only removes what came before it.
    assert_eq!(
        wall.inherits.keys().collect::<Vec<_>>(),
        ["Finish", "Type", "Style"]
    );
    assert_eq!(flat["site"].children["Slab"].as_deref(), Some("slab"));
}

#[test]
fn a_single_file_flattens_in_file_order() {
    let file = read("wall-with-type.ifcx");
    let flat = flatten(&file.data);
    assert_eq!(flat.len(), 4);
    let wall = &flat["f3a1c2d4-0000-4000-8000-000000000002"];
    // The second node for this path overrides only the height.
    assert_eq!(*wall.attributes["example::wall::height"], json!(3.0));
    assert_eq!(*wall.attributes["example::wall::layers"], json!(3));
    assert_eq!(
        wall.attributes.keys().next().map(String::as_str),
        Some("example::wall::height")
    );
}

/// A composed tree as JSON, in the shape upstream's `PostCompositionNode`
/// serialises to with its maps written as objects.
fn tree_json(node: &ComposedNode) -> Value {
    let attributes: Map<String, Value> = node
        .attributes
        .iter()
        .map(|(k, v)| (k.clone(), (**v).clone()))
        .collect();
    let children: Map<String, Value> = node
        .children
        .iter()
        .map(|(k, v)| (k.clone(), tree_json(v)))
        .collect();
    json!({"path": node.path, "attributes": attributes, "children": children})
}

fn compose_nodes(nodes: Value) -> Result<Composition, ComposeError> {
    let nodes: Vec<IfcxNode> = serde_json::from_value(nodes).unwrap();
    compose(&flatten(&nodes))
}

#[test]
fn occurrences_and_types_compose_to_the_upstream_tree() {
    let file = read("occurrence-type.ifcx");
    let composed = compose(&flatten(&file.data)).unwrap();
    assert_eq!(composed.roots(), ["site", "annotations"]);

    let actual = tree_json(&composed.root());
    let expected: Value = serde_json::from_str(&fixture("occurrence-type.composed.json")).unwrap();
    assert_eq!(actual, expected);
    // Same content and same key order.
    assert_eq!(actual.to_string(), expected.to_string());
}

#[test]
fn instances_share_type_subtrees_until_edited() {
    let file = read("occurrence-type.ifcx");
    let composed = compose(&flatten(&file.data)).unwrap();
    let get = |path: &str| composed.get(path).unwrap_or_else(|| panic!("{path}"));

    // Wall A takes the type's body as is: the same allocation.
    assert!(Arc::ptr_eq(get("wall-a/Body"), get("wall-type/Body")));
    assert!(Arc::ptr_eq(get("wall-a/Body"), get("wall-type-body")));
    assert!(Arc::ptr_eq(get("opening/Cut"), get("wall-type-body")));
    // Wall B edits its body, so it gets its own copy along that path only.
    assert!(!Arc::ptr_eq(get("wall-b/Body"), get("wall-type-body")));
    assert_eq!(*get("wall-b/Body").attributes["example::colour"], "red");
    assert_eq!(
        *get("wall-b/Body/Material").attributes["example::colour"],
        "dark grey"
    );
    // The edit does not leak into the type or into other instances.
    assert!(!get("wall-type-body")
        .attributes
        .contains_key("example::colour"));
    assert_eq!(
        *get("wall-b/Opening/Cut/Material").attributes["example::colour"],
        "light grey"
    );
    // Attribute values are shared even below a copied node.
    assert!(Arc::ptr_eq(
        &get("wall-b/Body").attributes["example::mesh"],
        &get("wall-type-body").attributes["example::mesh"]
    ));

    // A null child deletes an inherited one; a child path that does not
    // exist is ignored.
    assert!(composed.get("wall-b/Axis").is_none());
    assert!(composed.get("wall-b/Missing").is_none());
    assert!(composed.get("wall-a/Axis").is_some());
    // Local attributes win over inherited ones.
    assert_eq!(*get("wall-a").attributes["example::fireRating"], "EI60");
    assert_eq!(*get("wall-b").attributes["example::fireRating"], "EI30");
}

#[test]
fn cycles_are_errors_naming_the_cycle() {
    let cases = [
        (
            json!([{"path": "a", "inherits": {"t": "b"}}, {"path": "b", "inherits": {"t": "a"}}]),
            vec!["a", "b"],
        ),
        (
            json!([{"path": "r", "children": {"x": "a"}},
                   {"path": "a", "children": {"x": "b"}},
                   {"path": "b", "children": {"x": "c"}},
                   {"path": "c", "children": {"x": "a"}}]),
            vec!["a", "b", "c"],
        ),
        // Through a child reference, which upstream does not detect.
        (
            json!([{"path": "a", "children": {"x": "b/y"}}, {"path": "b", "children": {"y": "a"}}]),
            vec!["a", "b"],
        ),
        // Through a node that edits a child.
        (
            json!([{"path": "a", "children": {"x": "c"}},
                   {"path": "a/x", "inherits": {"t": "b"}},
                   {"path": "b", "children": {"y": "a"}},
                   {"path": "c"}]),
            vec!["a", "b"],
        ),
        (json!([{"path": "a", "inherits": {"self": "a"}}]), vec!["a"]),
    ];
    for (nodes, cycle) in cases {
        let error = compose_nodes(nodes.clone()).unwrap_err();
        let cycle = cycle.iter().map(|s| s.to_string()).collect();
        assert_eq!(error, ComposeError::Cycle { cycle }, "{nodes}");
    }
    let error = compose_nodes(json!([{"path": "a", "inherits": {"self": "a"}}])).unwrap_err();
    assert_eq!(error.to_string(), "reference cycle: a -> a");
}

#[test]
fn unknown_references_are_errors_naming_the_reference() {
    let error = compose_nodes(json!([{"path": "a", "children": {"x": "nowhere"}}])).unwrap_err();
    let unknown = |path: &str, reference: &str| ComposeError::UnknownReference {
        path: path.into(),
        reference: reference.into(),
    };
    assert_eq!(error, unknown("a", "nowhere"));
    assert_eq!(error.to_string(), "node a references unknown node nowhere");

    // A known node without the named child.
    let error = compose_nodes(json!([
        {"path": "t", "children": {"Body": "b"}},
        {"path": "b"},
        {"path": "a", "inherits": {"t": "t/Head"}}
    ]))
    .unwrap_err();
    assert_eq!(error, unknown("a", "t/Head"));

    // A layer composed without the layer it builds on.
    let file = read("layer-base.ifcx");
    let error = compose(&flatten(&file.data)).unwrap_err();
    assert_eq!(error, unknown("wall", "wall-type"));
}

#[test]
fn child_references_resolve_into_composed_children() {
    // `t/Body/Mesh` names a child of a child that `t` inherits.
    let composed = compose_nodes(json!([
        {"path": "base", "children": {"Body": "body"}},
        {"path": "body", "children": {"Mesh": "mesh"}},
        {"path": "mesh", "attributes": {"x::points": [1, 2]}},
        {"path": "t", "inherits": {"base": "base"}},
        {"path": "a", "children": {"Shape": "t/Body/Mesh"}}
    ]))
    .unwrap();
    assert_eq!(composed.roots(), ["a"]);
    let shape = composed.get("a/Shape").unwrap();
    assert_eq!(shape.path, "mesh");
    assert!(Arc::ptr_eq(shape, composed.get("mesh").unwrap()));
    // A trailing `/` resolves like upstream's GetChildNodeWithPath.
    assert!(composed.get("a/Shape/").is_some());
}

#[test]
fn deep_chains_do_not_recurse() {
    const DEPTH: usize = 200_000;
    let mut nodes: Vec<IfcxNode> = (0..DEPTH)
        .map(|i| {
            let next = (i + 1 < DEPTH).then(|| json!({"next": format!("n{}", i + 1)}));
            serde_json::from_value(json!({"path": format!("n{i}"), "children": next})).unwrap()
        })
        .collect();
    let composed = compose(&flatten(&nodes)).unwrap();
    assert_eq!(composed.roots(), ["n0"]);
    let deepest = "n0".to_owned() + &"/next".repeat(DEPTH - 1);
    let last = composed.get(&deepest).unwrap();
    assert_eq!(last.path, format!("n{}", DEPTH - 1));
    drop(composed);

    // A cycle at the end of a long chain is still found.
    let back = [("next".to_owned(), Some("n0".to_owned()))];
    nodes.last_mut().unwrap().children = Some(back.into_iter().collect());
    match compose(&flatten(&nodes)) {
        Err(ComposeError::Cycle { cycle }) => assert_eq!(cycle.len(), DEPTH),
        other => panic!("{other:?}"),
    }
}
