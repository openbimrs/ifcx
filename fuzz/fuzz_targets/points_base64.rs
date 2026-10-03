//! `points::base64` and `points::array` decoding. The input is split at the
//! first NUL byte into the positions and colours strings; it is also read
//! as a JSON `points::array` value.
#![no_main]

use libfuzzer_sys::fuzz_target;
use openbim_ifcx_geometry::PointCloud;
use serde_json::{json, Value};

fuzz_target!(|data: &[u8]| {
    let (positions, colors) = match data.iter().position(|&b| b == 0) {
        Some(i) => (&data[..i], Some(&data[i + 1..])),
        None => (data, None),
    };
    let mut value = json!({ "positions": String::from_utf8_lossy(positions) });
    if let Some(colors) = colors {
        value["colors"] = json!(String::from_utf8_lossy(colors));
    }
    if let Ok(cloud) = PointCloud::from_points_base64(&value) {
        let bytes = positions
            .iter()
            .filter(|b| !b.is_ascii_whitespace())
            .count();
        assert!(cloud.len() <= bytes / 16 + 1);
        if let Some(colors) = &cloud.colors {
            assert_eq!(colors.len(), cloud.len());
        }
    }

    if let Ok(value) = serde_json::from_slice::<Value>(data) {
        if let Ok(cloud) = PointCloud::from_points_array(&value) {
            if let Some(colors) = &cloud.colors {
                assert_eq!(colors.len(), cloud.len());
            }
        }
    }
});
