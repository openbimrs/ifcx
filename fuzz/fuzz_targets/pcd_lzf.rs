//! The `binary_compressed` path of the PCD reader with a fixed, valid
//! header, so every input exercises the LZF decoder and the column-major
//! layout. The first two bytes pick POINTS and the fields; the rest is the
//! body: compressed size, decompressed size, LZF data.
#![no_main]

use libfuzzer_sys::fuzz_target;
use openbim_ifcx_geometry::PointCloud;

fuzz_target!(|data: &[u8]| {
    let [points, layout, body @ ..] = data else {
        return;
    };
    let (fields, size, kind, count) = match layout % 4 {
        0 => ("x y z", "4 4 4", "F F F", "1 1 1"),
        1 => ("x y z rgb", "4 4 4 4", "F F F U", "1 1 1 1"),
        2 => ("normal_x x rgb y z", "4 4 4 4 4", "F F F F F", "1 1 1 1 1"),
        _ => ("x y z label", "4 4 4 2", "F F F U", "1 1 1 3"),
    };
    let mut pcd = format!(
        "VERSION 0.7\nFIELDS {fields}\nSIZE {size}\nTYPE {kind}\nCOUNT {count}\n\
         WIDTH {points}\nHEIGHT 1\nPOINTS {points}\nDATA binary_compressed\n"
    )
    .into_bytes();
    pcd.extend_from_slice(body);
    if let Ok(cloud) = PointCloud::from_pcd(&pcd) {
        assert_eq!(cloud.len(), usize::from(*points));
    }
});
