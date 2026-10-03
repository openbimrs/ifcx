//! GLB export of the hand-written fixtures under `tests/fixtures`, read back
//! with a minimal GLB parser.

use openbim_ifcx::{compose, flatten, IfcxFile};
use openbim_ifcx_geometry::glb::{to_glb, GlbOptions};
use openbim_ifcx_geometry::scene::{RenderScene, SceneOptions};
use openbim_ifcx_geometry::Vec3;
use serde_json::{json, Value};

fn scene(name: &str) -> RenderScene {
    let path = format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"));
    let file = IfcxFile::from_json_slice(&std::fs::read(path).unwrap()).unwrap();
    RenderScene::from_composition(
        &compose(&flatten(&file.data)).unwrap(),
        &SceneOptions::default(),
    )
}

struct Glb {
    json: Value,
    bin: Vec<u8>,
}

fn parse(glb: &[u8]) -> Glb {
    let u32_at = |i: usize| u32::from_le_bytes(glb[i..i + 4].try_into().unwrap()) as usize;
    assert_eq!(&glb[..4], b"glTF");
    assert_eq!(u32_at(4), 2, "version");
    assert_eq!(u32_at(8), glb.len(), "total length");
    assert_eq!(glb.len() % 4, 0);
    let json_len = u32_at(12);
    assert_eq!(json_len % 4, 0);
    assert_eq!(&glb[16..20], b"JSON");
    let json: Value = serde_json::from_slice(&glb[20..20 + json_len]).unwrap();
    let rest = &glb[20 + json_len..];
    let bin = if rest.is_empty() {
        Vec::new()
    } else {
        let bin_len = u32::from_le_bytes(rest[..4].try_into().unwrap()) as usize;
        assert_eq!(&rest[4..8], b"BIN\0");
        assert_eq!(bin_len % 4, 0);
        assert_eq!(rest.len(), 8 + bin_len);
        rest[8..].to_vec()
    };
    Glb { json, bin }
}

impl Glb {
    fn floats(&self, accessor: usize) -> Vec<[f32; 3]> {
        let a = &self.json["accessors"][accessor];
        assert_eq!(a["componentType"], 5126);
        assert_eq!(a["type"], "VEC3");
        let view = &self.json["bufferViews"][a["bufferView"].as_u64().unwrap() as usize];
        let offset = view["byteOffset"].as_u64().unwrap() as usize;
        let count = a["count"].as_u64().unwrap() as usize;
        assert_eq!(view["byteLength"].as_u64().unwrap() as usize, count * 12);
        (0..count)
            .map(|i| {
                [0, 1, 2].map(|k| {
                    let at = offset + i * 12 + k * 4;
                    f32::from_le_bytes(self.bin[at..at + 4].try_into().unwrap())
                })
            })
            .collect()
    }

    fn node_named<'a>(&'a self, name: &str) -> Vec<&'a Value> {
        self.json["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|n| n["name"] == name)
            .collect()
    }

    fn primitive(&self, node: &Value) -> &Value {
        &self.json["meshes"][node["mesh"].as_u64().unwrap() as usize]["primitives"][0]
    }
}

/// Applies a glTF column-major matrix to a point.
fn apply(m: &Value, p: [f32; 3]) -> Vec3 {
    let m: Vec<f64> = m
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    let p = p.map(f64::from);
    [0, 1, 2].map(|r| m[r] * p[0] + m[4 + r] * p[1] + m[8 + r] * p[2] + m[12 + r])
}

#[test]
fn instances_share_meshes_and_carry_ifcx_paths() {
    let scene = scene("instanced-types.ifcx");
    let glb = parse(&to_glb(&scene, &GlbOptions::default()).unwrap());
    let json = &glb.json;
    assert_eq!(json["asset"]["version"], "2.0");
    assert_eq!(json["scenes"][0]["nodes"], json!([0]));

    let nodes = json["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 1 + scene.instances.len());
    let names: Vec<_> = nodes[1..]
        .iter()
        .map(|n| n["name"].as_str().unwrap())
        .collect();
    let paths: Vec<_> = scene.instances.iter().map(|i| i.path.as_str()).collect();
    assert_eq!(names, paths);
    assert_eq!(nodes[0]["children"].as_array().unwrap().len(), names.len());

    // The three panels reference one glTF mesh.
    let panels: Vec<_> = ["building/Left", "building/Right", "building/Mirror"]
        .iter()
        .map(|n| glb.node_named(n)[0]["mesh"].clone())
        .collect();
    assert!(panels.iter().all(|m| *m == panels[0]));
    assert_eq!(
        glb.node_named("building/Left")[0]["extras"]["ifcxNode"],
        "left"
    );

    // Primitive modes and attributes per kind.
    let panel = glb.primitive(glb.node_named("building/Left")[0]);
    assert_eq!(panel["mode"], 4);
    assert!(panel["attributes"]["NORMAL"].is_u64());
    assert!(panel["indices"].is_u64());
    let axis = glb.primitive(glb.node_named("building/Wall/Axis")[0]);
    assert_eq!(axis["mode"], 1);
    let scan = glb.primitive(glb.node_named("building/Scan")[0]);
    assert_eq!(scan["mode"], 0);
    assert!(scan["attributes"]["COLOR_0"].is_u64());
    let raw = glb.primitive(glb.node_named("building/Scan/Raw")[0]);
    assert!(raw["attributes"].get("COLOR_0").is_none());

    // Materials: the panel's glTF material, the wall's colour.
    let material = |p: &Value| &json["materials"][p["material"].as_u64().unwrap() as usize];
    let glass = material(panel);
    assert_eq!(glass["alphaMode"], "BLEND");
    assert_eq!(
        glass["pbrMetallicRoughness"]["baseColorFactor"],
        json!([
            0.5,
            0.699999988079071,
            0.8999999761581421,
            0.4000000059604645
        ])
    );
    assert_eq!(glass["pbrMetallicRoughness"]["metallicFactor"], 0.0);
    let brick = material(axis);
    assert_eq!(
        brick["pbrMetallicRoughness"]["baseColorFactor"],
        json!([
            0.800000011920929,
            0.30000001192092896,
            0.20000000298023224,
            1.0
        ])
    );
    assert!(brick.get("alphaMode").is_none());

    // Buffer views are 4-byte aligned and inside the buffer; POSITION
    // accessors have exact min and max.
    assert_eq!(json["buffers"][0]["byteLength"], glb.bin.len());
    for view in json["bufferViews"].as_array().unwrap() {
        let offset = view["byteOffset"].as_u64().unwrap() as usize;
        assert_eq!(offset % 4, 0);
        assert!(offset + view["byteLength"].as_u64().unwrap() as usize <= glb.bin.len());
    }
    for mesh in json["meshes"].as_array().unwrap() {
        let position = mesh["primitives"][0]["attributes"]["POSITION"]
            .as_u64()
            .unwrap() as usize;
        let values = glb.floats(position);
        for k in 0..3 {
            let min = values.iter().map(|v| v[k]).fold(f32::INFINITY, f32::min);
            let max = values
                .iter()
                .map(|v| v[k])
                .fold(f32::NEG_INFINITY, f32::max);
            let accessor = &json["accessors"][position];
            assert_eq!(accessor["min"][k].as_f64().unwrap() as f32, min);
            assert_eq!(accessor["max"][k].as_f64().unwrap() as f32, max);
        }
    }
}

#[test]
fn placement_round_trips_through_root_and_node_matrices() {
    let scene = scene("georeferenced.ifcx");
    let glb = parse(&to_glb(&scene, &GlbOptions::default()).unwrap());
    let root = &glb.json["nodes"][0];
    assert_eq!(root["name"], "IFCX");
    let o = scene.origin;
    // Y-up: (x, y, z) in IFCX is (x, z, -y) in glTF.
    assert_eq!(root["translation"], json!([o[0], o[2], -o[1]]));
    assert_eq!(root["extras"]["ifcxOrigin"], json!(o));
    let rotation: Vec<f64> = root["rotation"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    assert!(
        (rotation[0] + 0.5f64.sqrt()).abs() < 1e-12 && (rotation[3] - 0.5f64.sqrt()).abs() < 1e-12
    );

    let node = glb.node_named("terrain/Surface")[0];
    let positions = glb.floats(
        glb.primitive(node)["attributes"]["POSITION"]
            .as_u64()
            .unwrap() as usize,
    );
    let local = apply(&node["matrix"], positions[0]);
    // Root: rotate, then translate.
    let gltf = [local[0] + o[0], local[2] + o[2], -(local[1] + o[1])];
    let expected = [512000.001, 101.5, -5612000.002];
    for k in 0..3 {
        assert!((gltf[k] - expected[k]).abs() < 1e-4, "{gltf:?}");
    }

    // Z-up and local: no rotation, no translation, origin in extras only.
    let options = GlbOptions::default()
        .with_y_up(false)
        .with_origin_on_root(false);
    let glb = parse(&to_glb(&scene, &options).unwrap());
    let root = &glb.json["nodes"][0];
    assert!(root.get("rotation").is_none());
    assert!(root.get("translation").is_none());
    assert_eq!(root["extras"]["ifcxUpAxis"], "Z");
    assert_eq!(root["extras"]["ifcxOrigin"], json!(o));
}

#[test]
fn placed_storey_matches_the_scene() {
    let scene = scene("placed-storey.ifcx");
    let glb = parse(&to_glb(&scene, &GlbOptions::default().with_y_up(false)).unwrap());
    let wall = glb.node_named("site/Storey/Wall")[0];
    let positions = glb.floats(
        glb.primitive(wall)["attributes"]["POSITION"]
            .as_u64()
            .unwrap() as usize,
    );
    let o = scene.origin;
    let world: Vec<Vec3> = positions
        .iter()
        .map(|&p| {
            let r = apply(&wall["matrix"], p);
            [r[0] + o[0], r[1] + o[1], r[2] + o[2]]
        })
        .collect();
    let expected = [
        [100.0, 200.0, 3.0],
        [100.0, 202.0, 3.0],
        [100.0, 200.0, 6.0],
    ];
    for (w, e) in world.iter().zip(expected) {
        assert!(w.iter().zip(e).all(|(a, b)| (a - b).abs() < 1e-4), "{w:?}");
    }
}
