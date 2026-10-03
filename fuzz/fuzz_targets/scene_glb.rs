//! `RenderScene::from_composition` and `to_glb` on composed files: IFCX
//! JSON, or structured node opinions whose attributes are geometry and
//! presentation values. The GLB must be structurally valid: header and
//! chunk lengths, 4-byte alignment, buffer views inside the binary chunk,
//! accessors inside their views, and indices inside their vertex count.
#![no_main]

use libfuzzer_sys::fuzz_target;
use openbim_ifcx::{compose, flatten};
use openbim_ifcx_fuzz::input_nodes;
use openbim_ifcx_geometry::{to_glb, GlbOptions, RenderScene, SceneOptions};
use serde_json::Value;

fn u32_at(bytes: &[u8], at: usize) -> usize {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize
}

fn check_glb(glb: &[u8]) {
    assert_eq!(&glb[0..4], b"glTF");
    assert_eq!(u32_at(glb, 4), 2);
    assert_eq!(u32_at(glb, 8), glb.len());
    assert_eq!(glb.len() % 4, 0);
    let json_len = u32_at(glb, 12);
    assert_eq!(json_len % 4, 0);
    assert_eq!(&glb[16..20], b"JSON");
    let json: Value = serde_json::from_slice(&glb[20..20 + json_len]).expect("JSON chunk");
    // The writer never means to write null; a non-finite number turns into
    // one, which glTF readers reject.
    let mut values = vec![&json];
    while let Some(value) = values.pop() {
        match value {
            Value::Null => panic!("null in the GLB JSON: {json}"),
            Value::Array(items) => values.extend(items),
            Value::Object(fields) => values.extend(fields.values()),
            _ => {}
        }
    }
    let rest = &glb[20 + json_len..];
    let bin = if rest.is_empty() {
        &[][..]
    } else {
        assert_eq!(&rest[4..8], b"BIN\0");
        assert_eq!(u32_at(rest, 0), rest.len() - 8);
        &rest[8..]
    };
    let Some(views) = json["bufferViews"].as_array() else {
        assert!(bin.is_empty());
        return;
    };
    assert_eq!(
        json["buffers"][0]["byteLength"].as_u64(),
        Some(bin.len() as u64)
    );
    let views: Vec<(usize, usize)> = views
        .iter()
        .map(|v| {
            let offset = v["byteOffset"].as_u64().unwrap() as usize;
            let len = v["byteLength"].as_u64().unwrap() as usize;
            assert_eq!(offset % 4, 0);
            assert!(offset + len <= bin.len());
            (offset, len)
        })
        .collect();
    let accessors = json["accessors"].as_array().expect("accessors");
    for accessor in accessors {
        let (_, len) = views[accessor["bufferView"].as_u64().unwrap() as usize];
        let count = accessor["count"].as_u64().unwrap() as usize;
        let width = if accessor["type"] == "VEC3" { 12 } else { 4 };
        assert_eq!(count * width, len);
    }
    for mesh in json["meshes"].as_array().expect("meshes") {
        let primitive = &mesh["primitives"][0];
        let position = &accessors[primitive["attributes"]["POSITION"].as_u64().unwrap() as usize];
        let vertices = position["count"].as_u64().unwrap() as u32;
        if let Some(i) = primitive["indices"].as_u64() {
            let (offset, len) =
                views[accessors[i as usize]["bufferView"].as_u64().unwrap() as usize];
            for index in bin[offset..offset + len].chunks_exact(4) {
                assert!(u32::from_le_bytes(index.try_into().unwrap()) < vertices);
            }
        }
    }
}

fuzz_target!(|data: &[u8]| {
    let Some(nodes) = input_nodes(data, true) else {
        return;
    };
    let Ok(composition) = compose(&flatten(&nodes)) else {
        return;
    };
    // Lower limits than the defaults keep each run fast under ASan; the
    // limits themselves are part of what is exercised.
    let options = SceneOptions::default()
        .with_max_visits(100_000)
        .with_max_path_bytes(16 << 20);
    let scene = RenderScene::from_composition(&composition, &options);
    assert!(scene.instances.len() <= 100_000);
    for instance in &scene.instances {
        assert!(!scene.instances_at(&instance.path).is_empty());
    }
    let options = GlbOptions::default().with_y_up(data.len() % 2 == 0);
    let glb = to_glb(&scene, &options).expect("small scenes fit in a GLB");
    check_glb(&glb);
});
