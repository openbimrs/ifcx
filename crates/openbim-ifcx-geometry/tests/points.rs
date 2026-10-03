//! Point clouds from hand-written `ifcx_alpha` samples. Every encoding holds
//! the same three points, coloured red, green, and blue.

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use openbim_ifcx::IfcxFile;
use openbim_ifcx_geometry::{DecodeError, PointCloud};
use serde_json::{json, Value};

/// Exact in `f32`, so every encoding can hold them.
const POSITIONS: [[f64; 3]; 3] = [[0.0, 0.0, 0.0], [1.0, 2.0, 3.0], [0.5, -0.25, 4.0]];
const COLORS: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
/// `POSITIONS` and `COLORS` as little-endian f32 triples.
const POSITIONS_B64: &str = "AAAAAAAAAAAAAAAAAACAPwAAAEAAAEBAAAAAPwAAgL4AAIBA";
const COLORS_B64: &str = "AACAPwAAAAAAAAAAAAAAAAAAgD8AAAAAAAAAAAAAAAAAAIA/";
/// `COLORS` packed as PCD `rgb` (`0x00RRGGBB`).
const PACKED: [u32; 3] = [0x00ff_0000, 0x0000_ff00, 0x0000_00ff];

fn expected() -> PointCloud {
    PointCloud {
        positions: POSITIONS.to_vec(),
        colors: Some(COLORS.to_vec()),
    }
}

fn f32_bytes(v: f64) -> [u8; 4] {
    (v as f32).to_le_bytes()
}

fn pcd_header(fields: &str, size: &str, ty: &str, data: &str) -> Vec<u8> {
    format!(
        "# .PCD v0.7 - Point Cloud Data file format\nVERSION 0.7\nFIELDS {fields}\n\
         SIZE {size}\nTYPE {ty}\nCOUNT {}\nWIDTH 3\nHEIGHT 1\n\
         VIEWPOINT 0 0 0 1 0 0 0\nPOINTS 3\nDATA {data}\n",
        vec!["1"; fields.split(' ').count()].join(" ")
    )
    .into_bytes()
}

fn pcd_ascii() -> Vec<u8> {
    let mut pcd = pcd_header("x y z rgb", "4 4 4 4", "F F F U", "ascii");
    for (p, c) in POSITIONS.iter().zip(PACKED) {
        pcd.extend(format!("{} {} {} {c}\n", p[0], p[1], p[2]).bytes());
    }
    pcd
}

fn pcd_binary() -> Vec<u8> {
    let mut pcd = pcd_header("x y z rgb", "4 4 4 4", "F F F U", "binary");
    for (p, c) in POSITIONS.iter().zip(PACKED) {
        for &v in p {
            pcd.extend(f32_bytes(v));
        }
        pcd.extend(c.to_le_bytes());
    }
    pcd
}

/// Column-major data (all x, all y, all z, all rgb), stored as LZF literal
/// runs of at most 32 bytes.
fn pcd_binary_compressed() -> Vec<u8> {
    let mut raw = Vec::new();
    for axis in 0..3 {
        for p in POSITIONS {
            raw.extend(f32_bytes(p[axis]));
        }
    }
    for c in PACKED {
        raw.extend(c.to_le_bytes());
    }
    let mut lzf = Vec::new();
    for run in raw.chunks(32) {
        lzf.push(run.len() as u8 - 1);
        lzf.extend(run);
    }
    let mut pcd = pcd_header("x y z rgb", "4 4 4 4", "F F F U", "binary_compressed");
    pcd.extend((lzf.len() as u32).to_le_bytes());
    pcd.extend((raw.len() as u32).to_le_bytes());
    pcd.extend(lzf);
    pcd
}

fn node(attributes: Value) -> IfcxFile {
    let file = json!({
        "header": {"id": "points", "ifcxVersion": "ifcx_alpha", "dataVersion": "1",
                   "author": "test", "timestamp": "2026-10-03"},
        "imports": [],
        "schemas": {},
        "data": [{"path": "cloud", "attributes": attributes}]
    });
    IfcxFile::from_json_str(&file.to_string()).unwrap()
}

fn decode(attributes: Value) -> Result<PointCloud, DecodeError> {
    let file = node(attributes);
    PointCloud::from_attributes(&file.data[0]).map(|cloud| cloud.expect("a point cloud"))
}

#[test]
fn every_encoding_decodes_to_the_same_cloud() {
    let array = decode(json!({"points::array": {
        "positions": [[0, 0, 0], [1, 2, 3], [0.5, -0.25, 4]],
        "colors": [[1, 0, 0], [0, 1, 0], [0, 0, 1]],
    }}))
    .unwrap();
    let base64 = decode(json!({"points::base64": {
        "positions": POSITIONS_B64, "colors": COLORS_B64,
    }}))
    .unwrap();
    assert_eq!(array, expected());
    assert_eq!(base64, expected());
    for pcd in [pcd_ascii(), pcd_binary(), pcd_binary_compressed()] {
        let encoded = STANDARD.encode(&pcd);
        assert_eq!(decode(json!({"pcd::base64": encoded})).unwrap(), expected());
        assert_eq!(PointCloud::from_pcd(&pcd).unwrap(), expected());
    }
}

#[test]
fn array_positions_keep_full_precision() {
    // A georeferenced coordinate that f32 would round to 5_432_100.0.
    let cloud = decode(json!({"points::array": {"positions": [[5_432_100.125, 0, 0]]}})).unwrap();
    assert_eq!(cloud.positions, [[5_432_100.125, 0.0, 0.0]]);
}

#[test]
fn colours_are_optional() {
    let array = decode(json!({"points::array": {
        "positions": [[0, 0, 0], [1, 2, 3], [0.5, -0.25, 4]]
    }}))
    .unwrap();
    let base64 = decode(json!({"points::base64": {"positions": POSITIONS_B64}})).unwrap();
    let mut pcd = pcd_header("x y z", "4 4 4", "F F F", "ascii");
    pcd.extend(b"0 0 0\n1 2 3\n0.5 -0.25 4\n");
    for cloud in [array, base64, PointCloud::from_pcd(&pcd).unwrap()] {
        assert_eq!(cloud.positions, POSITIONS);
        assert_eq!(cloud.colors, None);
    }
}

#[test]
fn nodes_without_point_attributes_have_no_cloud() {
    let file = node(json!({"bsi::ifc::presentation::diffuseColor": [1, 0, 0]}));
    assert_eq!(PointCloud::from_attributes(&file.data[0]), Ok(None));
    let file = node(json!({"points::array": null}));
    assert_eq!(PointCloud::from_attributes(&file.data[0]), Ok(None));
}

#[test]
fn pcd_wins_over_array_which_wins_over_base64() {
    let one = json!({"positions": [[9, 9, 9]]});
    let cloud = decode(json!({
        "points::base64": {"positions": "AAAAAAAAAAAAAAAA"},
        "points::array": one,
        "pcd::base64": STANDARD.encode(pcd_ascii()),
    }))
    .unwrap();
    assert_eq!(cloud, expected());
    let cloud = decode(json!({
        "points::base64": {"positions": "AAAAAAAAAAAAAAAA"},
        "points::array": one,
    }))
    .unwrap();
    assert_eq!(cloud.positions, [[9.0, 9.0, 9.0]]);
}

#[test]
fn base64_accepts_what_atob_accepts() {
    // Whitespace anywhere and missing padding are fine.
    let spaced = format!(" {}\n{} ", &POSITIONS_B64[..20], &POSITIONS_B64[20..]);
    let cloud = decode(json!({"points::base64": {"positions": spaced}})).unwrap();
    assert_eq!(cloud.positions, POSITIONS);
}

#[test]
fn large_base64_clouds_decode_across_chunks() {
    let positions: Vec<[f64; 3]> = (0..10_000)
        .map(|i| [f64::from(i), -f64::from(i), 0.25 * f64::from(i)])
        .collect();
    let bytes: Vec<u8> = positions
        .iter()
        .flatten()
        .flat_map(|&v| f32_bytes(v))
        .collect();
    let cloud = decode(json!({"points::base64": {"positions": STANDARD.encode(bytes)}})).unwrap();
    assert_eq!(cloud.positions, positions);
    // Allocated once at the final size.
    assert_eq!(cloud.positions.capacity(), positions.len());
}

fn error(attributes: Value) -> DecodeError {
    decode(attributes).expect_err("malformed input")
}

#[test]
fn malformed_values_are_typed_errors() {
    assert!(matches!(
        error(json!({"points::array": {"positions": [[0, 0]]}})),
        DecodeError::WrongShape { at, .. } if at == "positions[0]"
    ));
    assert_eq!(
        error(json!({"points::array": {"colors": []}})),
        DecodeError::MissingField { field: "positions" }
    );
    assert_eq!(
        error(json!({"points::array": {"positions": [[0, 0, 0]], "colors": []}})),
        DecodeError::LengthMismatch {
            field: "colors",
            len: 0,
            expected: 1
        }
    );
    assert!(matches!(
        error(json!({"points::array": [[0, 0, 0]]})),
        DecodeError::WrongShape { at, .. } if at == "points::array"
    ));
    assert!(matches!(
        error(json!({"points::base64": {"positions": 3}})),
        DecodeError::WrongShape { at, .. } if at == "positions"
    ));
    assert!(matches!(
        error(json!({"pcd::base64": ["not", "a", "string"]})),
        DecodeError::WrongShape { at, .. } if at == "pcd::base64"
    ));
}

#[test]
fn malformed_base64_is_a_typed_error() {
    assert!(matches!(
        error(json!({"points::base64": {"positions": "AAAA*AAAAAAAAAAA"}})),
        DecodeError::InvalidBase64 { at, .. } if at == "positions"
    ));
    assert!(matches!(
        error(json!({"points::base64": {"positions": "AAAAA"}})),
        DecodeError::InvalidBase64 { .. }
    ));
    // Eight bytes: two floats, not a triple.
    assert!(matches!(
        error(json!({"points::base64": {"positions": "AAAAAAAAAAA="}})),
        DecodeError::ByteLength {
            bytes: 8,
            element_size: 12,
            ..
        }
    ));
    assert!(matches!(
        error(
            json!({"points::base64": {"positions": POSITIONS_B64, "colors": "AAAAAAAAAAAAAAAA"}})
        ),
        DecodeError::LengthMismatch {
            field: "colors",
            len: 1,
            expected: 3
        }
    ));
    assert!(matches!(
        error(json!({"pcd::base64": "%%%%"})),
        DecodeError::InvalidBase64 { at, .. } if at == "pcd::base64"
    ));
}

fn pcd_error(pcd: &[u8]) -> DecodeError {
    PointCloud::from_pcd(pcd).expect_err("malformed PCD")
}

#[test]
fn malformed_pcd_is_a_typed_error() {
    let invalid = |e: DecodeError| matches!(e, DecodeError::InvalidPcd { .. });
    assert!(
        invalid(pcd_error(b"VERSION 0.7\nFIELDS x y z\n")),
        "no DATA"
    );
    assert!(
        invalid(pcd_error(b"VERSION 0.7\nDATA ascii\n")),
        "no FIELDS"
    );
    assert!(invalid(pcd_error(
        b"FIELDS x y z\nSIZE 4 4\nTYPE F F F\nPOINTS 1\nDATA binary\n"
    )));
    assert!(invalid(pcd_error(b"FIELDS x y\nDATA ascii\n0 0\n")), "no z");
    assert!(invalid(pcd_error(
        b"FIELDS x y z\nPOINTS many\nDATA ascii\n"
    )));
    assert!(invalid(pcd_error(b"FIELDS x y z\nDATA ascii\n0 0 zero\n")));
    assert!(invalid(pcd_error(b"FIELDS x y z\nDATA ascii\n0 0\n")));

    let mut truncated = pcd_binary();
    truncated.truncate(truncated.len() - 1);
    assert!(invalid(pcd_error(&truncated)));

    let mut compressed = pcd_binary_compressed();
    compressed.truncate(compressed.len() - 1);
    assert!(invalid(pcd_error(&compressed)));
    let header_end = pcd_header("x y z rgb", "4 4 4 4", "F F F U", "binary_compressed").len();
    let mut lying = pcd_binary_compressed();
    lying[header_end + 4..header_end + 8].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(invalid(pcd_error(&lying)));
}

#[test]
fn unsupported_pcd_layouts_are_reported_as_such() {
    let unsupported = |e: DecodeError| matches!(e, DecodeError::UnsupportedPcd { .. });
    let mut doubles = pcd_header("x y z", "8 8 8", "F F F", "binary");
    doubles.extend([0; 72]);
    assert!(unsupported(pcd_error(&doubles)));
    assert!(unsupported(pcd_error(&pcd_header(
        "x y z",
        "4 4 4",
        "F F F",
        "binary_lz4"
    ))));
    let mut short_rgb = pcd_header("x y z rgb", "4 4 4 2", "F F F U", "binary");
    short_rgb.extend([0; 42]);
    assert!(unsupported(pcd_error(&short_rgb)));
}

#[test]
fn pcd_reads_like_three_js() {
    // Comments, an unread field, a float-typed rgb holding packed bits,
    // and CRLF line ends.
    let red_bits = f32::from_bits(0x00ff_0000);
    let pcd = format!(
        "# comment\r\nFIELDS x y z rgb intensity\r\nSIZE 4 4 4 4 4\r\nTYPE F F F F F\r\n\
         COUNT 1 1 1 1 1\r\nWIDTH 1\r\nHEIGHT 1\r\nPOINTS 1\r\nDATA ascii\r\n\
         1 2 3 {red_bits:e} 0.5\r\n\r\n"
    );
    let cloud = PointCloud::from_pcd(pcd.as_bytes()).unwrap();
    assert_eq!(cloud.positions, [[1.0, 2.0, 3.0]]);
    assert_eq!(cloud.colors, Some(vec![[1.0, 0.0, 0.0]]));

    // ASCII coordinates go through f32 unless declared as 8-byte values.
    let pcd = b"FIELDS x y z\nSIZE 4 4 8\nTYPE F F F\nDATA ascii\n0.1 0 0.1\n";
    let cloud = PointCloud::from_pcd(pcd).unwrap();
    assert_eq!(cloud.positions, [[f64::from(0.1f32), 0.0, 0.1]]);

    // Binary rows skip unread fields; POINTS falls back to WIDTH × HEIGHT.
    let mut pcd =
        b"FIELDS intensity x y z\nSIZE 2 4 4 4\nTYPE U F F F\nWIDTH 1\nHEIGHT 2\nDATA binary\n"
            .to_vec();
    for p in [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]] {
        pcd.extend([7, 7]);
        pcd.extend(p.into_iter().flat_map(f32_bytes));
    }
    let cloud = PointCloud::from_pcd(&pcd).unwrap();
    assert_eq!(cloud.positions, [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]]);
    assert_eq!(cloud.colors, None);
}
