//! Composition of hand-written layers. Expected values were checked against
//! upstream's TypeScript implementation (buildingSMART/IFC5-development
//! `src/ifcx-core/composition`) run on the same fixtures.

use std::path::PathBuf;

use indexmap::IndexMap;
use openbim_ifcx::{flatten, FlatNode, IfcxFile, IfcxNode};
use serde_json::{json, Map, Value};

fn read(name: &str) -> IfcxFile {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "tests", "fixtures", name]
        .iter()
        .collect();
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    IfcxFile::from_json_str(&text).unwrap_or_else(|e| panic!("{name}: {e}"))
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
