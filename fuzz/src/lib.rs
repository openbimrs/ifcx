//! Input generators and checks shared by the fuzz targets.
//!
//! Targets that take a whole file accept two encodings, chosen by the first
//! byte, so that one corpus can hold both:
//!
//! - input starting with `{` is IFCX JSON, read with
//!   [`IfcxFile::from_json_slice`]; the hand-written fixtures seed this half;
//! - anything else drives [`arbitrary`] to build node lists over a small
//!   vocabulary of paths and names, so that references hit each other,
//!   form cycles, diamonds, and `head/child` edits far more often than
//!   mutated JSON would.

use std::collections::HashSet;
use std::sync::Arc;

use arbitrary::{Result, Unstructured};
use openbim_ifcx::{ComposedNode, Composition, IfcxFile, IfcxNode};
use serde_json::{json, Map, Value};

/// Head paths the structured generator uses.
const HEADS: usize = 12;
/// Child and inherit names the structured generator uses.
const NAMES: usize = 4;

fn head(u: &mut Unstructured<'_>) -> Result<String> {
    Ok(format!("h{}", u.choose_index(HEADS)?))
}

fn name(u: &mut Unstructured<'_>) -> Result<String> {
    Ok(format!("c{}", u.choose_index(NAMES)?))
}

/// A node path or reference: a head, sometimes followed by child names.
fn path(u: &mut Unstructured<'_>) -> Result<String> {
    let mut path = head(u)?;
    for _ in 0..u.int_in_range(0..=3u8)?.saturating_sub(1) {
        path.push('/');
        path.push_str(&name(u)?);
    }
    Ok(path)
}

fn optional_path(u: &mut Unstructured<'_>) -> Result<Value> {
    Ok(if u.ratio(1, 6)? {
        Value::Null
    } else {
        Value::String(path(u)?)
    })
}

fn number(u: &mut Unstructured<'_>) -> Result<Value> {
    Ok(match u.choose_index(6)? {
        0 => json!(u.int_in_range(-2..=3i64)?),
        1 => json!(f64::from(u.arbitrary::<i16>()?) / 8.0),
        2 => json!(u.arbitrary::<u32>()?),
        3 => json!(u.arbitrary::<i64>()?),
        4 => serde_json::Number::from_f64(u.arbitrary::<f64>()?).map_or(json!(0), Value::Number),
        _ => json!(1e300),
    })
}

fn point(u: &mut Unstructured<'_>) -> Result<Value> {
    if u.ratio(1, 20)? {
        // A malformed point: wrong arity or a non-number.
        return Ok(json!([number(u)?, "x"]));
    }
    Ok(json!([number(u)?, number(u)?, number(u)?]))
}

fn points(u: &mut Unstructured<'_>, max: usize) -> Result<Vec<Value>> {
    let n = u.int_in_range(0..=max)?;
    (0..n).map(|_| point(u)).collect()
}

fn small_ints(u: &mut Unstructured<'_>, max_len: usize, max: i64) -> Result<Vec<Value>> {
    let n = u.int_in_range(0..=max_len)?;
    (0..n)
        .map(|_| Ok(json!(u.int_in_range(-1..=max)?)))
        .collect()
}

fn base64_f32s(u: &mut Unstructured<'_>) -> Result<String> {
    use base64::Engine;
    let n = u.int_in_range(0..=12usize)?;
    let mut bytes = Vec::with_capacity(n * 12);
    for _ in 0..n * 3 {
        bytes.extend((f32::from(u.arbitrary::<i8>()?) / 4.0).to_le_bytes());
    }
    if u.ratio(1, 10)? {
        bytes.pop();
    }
    Ok(base64::engine::general_purpose::STANDARD.encode(bytes))
}

/// A value for one geometry or presentation attribute, mostly well formed.
fn attribute(u: &mut Unstructured<'_>) -> Result<(String, Value)> {
    let (id, value) = match u.choose_index(11)? {
        0 => {
            let mut rows = vec![
                json!([1, 0, 0, 0]),
                json!([0, 1, 0, 0]),
                json!([0, 0, 1, 0]),
                json!([number(u)?, number(u)?, number(u)?, 1]),
            ];
            if u.ratio(1, 4)? {
                let (i, j) = (u.choose_index(4)?, u.choose_index(4)?);
                rows[i][j] = number(u)?;
            }
            ("usd::xformop", json!({ "transform": rows }))
        }
        1 => {
            let points = points(u, 8)?;
            let max = points.len() as i64;
            let mut mesh = json!({
                "points": points,
                "faceVertexIndices": small_ints(u, 12, max)?,
            });
            if u.ratio(1, 4)? {
                mesh["faceVertexCounts"] = Value::Array(small_ints(u, 4, 4)?);
            }
            ("usd::usdgeom::mesh", mesh)
        }
        2 => {
            let mut curve = json!({ "points": points(u, 8)? });
            if u.ratio(1, 3)? {
                curve["curveVertexCounts"] = Value::Array(small_ints(u, 4, 5)?);
            }
            if u.ratio(1, 3)? {
                curve["wrap"] = json!(*u.choose(&["periodic", "nonperiodic", "pinned", "x"])?);
            }
            if u.ratio(1, 6)? {
                curve["type"] = json!(*u.choose(&["linear", "cubic"])?);
            }
            ("usd::usdgeom::basiscurves", curve)
        }
        3 => {
            let positions = points(u, 6)?;
            let mut cloud = json!({ "positions": positions });
            if u.ratio(1, 2)? {
                cloud["colors"] = Value::Array(points(u, 6)?);
            }
            ("points::array", cloud)
        }
        4 => {
            let mut cloud = json!({ "positions": base64_f32s(u)? });
            if u.ratio(1, 2)? {
                cloud["colors"] = json!(base64_f32s(u)?);
            }
            ("points::base64", cloud)
        }
        5 => {
            use base64::Engine;
            let n = u.int_in_range(0..=4usize)?;
            let mut pcd = format!(
                "FIELDS x y z rgb\nSIZE 4 4 4 4\nTYPE F F F U\nCOUNT 1 1 1 1\nPOINTS {n}\nDATA {}\n",
                u.choose(&["ascii", "binary"])?
            )
            .into_bytes();
            let len = u.int_in_range(0..=n * 16)?;
            pcd.extend(u.bytes(len)?);
            (
                "pcd::base64",
                json!(base64::engine::general_purpose::STANDARD.encode(pcd)),
            )
        }
        6 => (
            "bsi::ifc::presentation::diffuseColor",
            json!([number(u)?, number(u)?, number(u)?]),
        ),
        7 => ("bsi::ifc::presentation::opacity", number(u)?),
        8 => (
            "usd::usdgeom::visibility",
            json!({ "visibility": *u.choose(&["invisible", "inherited", "visible"])? }),
        ),
        9 => (
            "gltf::material",
            json!({
                "pbrMetallicRoughness": {
                    "baseColorFactor": [number(u)?, number(u)?, number(u)?, number(u)?],
                    "metallicFactor": number(u)?,
                },
                "alphaMode": *u.choose(&["OPAQUE", "MASK", "BLEND", "x"])?,
                "alphaCutoff": number(u)?,
            }),
        ),
        _ => (
            "x::value",
            json!({ "n": number(u)?, "s": u.arbitrary::<String>()? }),
        ),
    };
    Ok((id.to_owned(), value))
}

/// One structured node opinion as IFCX JSON.
fn node(u: &mut Unstructured<'_>, geometry: bool) -> Result<Value> {
    let mut node = Map::new();
    node.insert("path".into(), Value::String(path(u)?));
    for field in ["children", "inherits"] {
        if u.ratio(1, 2)? {
            let mut map = Map::new();
            for _ in 0..u.int_in_range(0..=3u8)? {
                map.insert(name(u)?, optional_path(u)?);
            }
            node.insert(field.into(), Value::Object(map));
        }
    }
    if u.ratio(1, 2)? {
        let mut attributes = Map::new();
        for _ in 0..u.int_in_range(0..=3u8)? {
            let (id, value) = if geometry {
                attribute(u)?
            } else {
                (format!("x::a{}", u.choose_index(4)?), number(u)?)
            };
            attributes.insert(id, value);
        }
        node.insert("attributes".into(), Value::Object(attributes));
    }
    Ok(Value::Object(node))
}

/// Structured node opinions; with `geometry`, attributes are drawn from the
/// geometry and presentation attributes the scene reads.
pub fn arbitrary_nodes(u: &mut Unstructured<'_>, geometry: bool) -> Result<Vec<IfcxNode>> {
    let mut nodes = Vec::new();
    while nodes.len() < 64 && !u.is_empty() {
        let value = node(u, geometry)?;
        nodes.push(serde_json::from_value(value).expect("generated nodes are valid IFCX nodes"));
    }
    Ok(nodes)
}

/// Wraps nodes and schemas in a file.
pub fn file(nodes: Vec<Value>, schemas: Map<String, Value>) -> IfcxFile {
    serde_json::from_value(json!({
        "header": {"id": "fuzz", "ifcxVersion": "ifcx_alpha", "dataVersion": "1",
                   "author": "fuzz", "timestamp": "2026-10-03"},
        "imports": [],
        "schemas": schemas,
        "data": nodes,
    }))
    .expect("generated files are valid IFCX files")
}

/// The input's nodes: IFCX JSON when it starts with `{`, structured
/// otherwise. `None` when the JSON is not an IFCX file.
pub fn input_nodes(data: &[u8], geometry: bool) -> Option<Vec<IfcxNode>> {
    if data.first() == Some(&b'{') {
        IfcxFile::from_json_slice(data).ok().map(|file| file.data)
    } else {
        arbitrary_nodes(&mut Unstructured::new(data), geometry).ok()
    }
}

/// Checks a composition's invariants without visiting a shared sub-tree
/// twice, so a diamond-shaped tree costs its distinct nodes only.
pub fn check_composition(composition: &Composition) {
    for root in composition.roots() {
        assert!(!root.contains('/'), "root {root:?} is not a head path");
        let node = composition.get(root).expect("every root resolves");
        assert_eq!(&node.path, root);
    }
    let root = composition.root();
    assert_eq!(root.children.len(), composition.roots().len());
    let mut seen: HashSet<*const ComposedNode> = HashSet::new();
    let mut stack: Vec<&Arc<ComposedNode>> = root.children.values().collect();
    while let Some(node) = stack.pop() {
        if !seen.insert(Arc::as_ptr(node)) {
            continue;
        }
        assert!(seen.len() <= 1_000_000, "composition is unexpectedly large");
        stack.extend(node.children.values());
    }
}
