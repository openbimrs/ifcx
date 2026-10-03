//! `PointCloud::from_pcd` on arbitrary bytes: header parsing and the
//! `ascii`, `binary`, and `binary_compressed` (LZF) bodies. The base64 entry
//! point must agree with the byte one.
#![no_main]

use base64::Engine;
use libfuzzer_sys::fuzz_target;
use openbim_ifcx_geometry::PointCloud;
use serde_json::Value;

fuzz_target!(|data: &[u8]| {
    let decoded = PointCloud::from_pcd(data);
    if let Ok(cloud) = &decoded {
        if let Some(colors) = &cloud.colors {
            assert_eq!(colors.len(), cloud.positions.len());
        }
    }
    if data.len() <= 4096 {
        let encoded = base64::engine::general_purpose::STANDARD.encode(data);
        // Compared as text: NaN coordinates are never equal to themselves.
        let via_base64 = PointCloud::from_pcd_base64(&Value::String(encoded));
        assert_eq!(format!("{via_base64:?}"), format!("{decoded:?}"));
    }
});
