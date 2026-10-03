//! Point clouds from `points::array`, `points::base64`, and `pcd::base64`
//! (`ifcx_alpha`).
//!
//! Decoding follows the upstream reference viewer (`src/viewer/render.ts`
//! and three.js `PCDLoader` r171 at buildingSMART/IFC5-development
//! `1a63082`):
//!
//! | Attribute | Value | Binary layout |
//! | --- | --- | --- |
//! | `points::array` | `{positions: [[x, y, z], …], colors?: [[r, g, b], …]}` | — |
//! | `points::base64` | `{positions: "<base64>", colors?: "<base64>"}` | little-endian `f32` triples back to back: 12-byte stride, no header |
//! | `pcd::base64` | `"<base64 of a .pcd file>"` | PCD v0.7 `ascii`, `binary`, or `binary_compressed`, fields `x y z` and optional `rgb` |
//!
//! When a node carries several of them, [`PointCloud::from_attributes`] uses
//! the first of `pcd::base64`, `points::array`, `points::base64`, the order
//! of the upstream viewer.
//!
//! # Numbers
//!
//! Positions are `f64` like every coordinate in this crate (see
//! [`crate::math`]). `points::array` keeps the JSON numbers exactly, where the
//! upstream viewer narrows them to `f32`; the `f32` values of the binary
//! encodings widen losslessly, and ASCII PCD coordinates are rounded to `f32`
//! as the loader does unless the header declares `SIZE 8`. Colours are `f32` linear RGB in `0..=1`: array
//! and base64 colours are used as written, and 8-bit PCD `rgb` is converted
//! from sRGB to linear as three.js does.
//!
//! # Copies
//!
//! `points::base64` decodes in fixed 12 KiB chunks straight into the output
//! vector, which is allocated once at its final size, so the decoded bytes
//! never exist as a second full-size buffer. `pcd::base64` decodes the file
//! bytes once, since its header and layout are only known after decoding;
//! `binary_compressed` additionally needs the LZF output. Points are then read
//! from that buffer without further copies. Base64 text with embedded ASCII
//! whitespace, which browsers' `atob` accepts, is first copied without it.

use std::borrow::Cow;

use base64::alphabet;
use base64::engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig};
use base64::engine::{DecodePaddingMode, Engine};
use base64::DecodeSliceError;
use serde_json::Value;

use crate::json::{object, points3};
use crate::math::Vec3;
use crate::{Attributes, DecodeError};

/// Attribute id of a point cloud given as JSON arrays.
pub const POINTS_ARRAY: &str = "points::array";
/// Attribute id of a point cloud given as base64 `f32` buffers.
pub const POINTS_BASE64: &str = "points::base64";
/// Attribute id of a point cloud given as a base64 PCD file.
pub const PCD_BASE64: &str = "pcd::base64";

/// Decoded points of one node, in the node's local coordinates.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PointCloud {
    /// Point positions.
    pub positions: Vec<Vec3>,
    /// One linear RGB colour in `0..=1` per position, when the source has
    /// colours.
    pub colors: Option<Vec<[f32; 3]>>,
}

impl PointCloud {
    /// Number of points.
    pub fn len(&self) -> usize {
        self.positions.len()
    }

    /// Whether the cloud has no points.
    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// Decodes the node's point cloud, or `None` when it has no point-cloud
    /// attribute. With several, `pcd::base64` wins over `points::array`, which
    /// wins over `points::base64`, as in the upstream viewer.
    pub fn from_attributes<A: Attributes + ?Sized>(
        attributes: &A,
    ) -> Result<Option<Self>, DecodeError> {
        if let Some(value) = attributes.attribute(PCD_BASE64) {
            Self::from_pcd_base64(value).map(Some)
        } else if let Some(value) = attributes.attribute(POINTS_ARRAY) {
            Self::from_points_array(value).map(Some)
        } else if let Some(value) = attributes.attribute(POINTS_BASE64) {
            Self::from_points_base64(value).map(Some)
        } else {
            Ok(None)
        }
    }

    /// Decodes a `points::array` value:
    /// `{"positions": [[x, y, z], …], "colors": [[r, g, b], …]}` with
    /// `colors` optional.
    pub fn from_points_array(value: &Value) -> Result<Self, DecodeError> {
        let fields = object(value, POINTS_ARRAY)?;
        let positions = points3(positions(fields)?, "positions")?;
        let colors = match fields.get("colors").filter(|v| !v.is_null()) {
            Some(colors) => Some(
                points3(colors, "colors")?
                    .into_iter()
                    .map(|c| c.map(|v| v as f32))
                    .collect(),
            ),
            None => None,
        };
        Self::checked(positions, colors)
    }

    /// Decodes a `points::base64` value:
    /// `{"positions": "<base64>", "colors": "<base64>"}` with `colors`
    /// optional. Each string holds little-endian `f32` triples back to back.
    pub fn from_points_base64(value: &Value) -> Result<Self, DecodeError> {
        let fields = object(value, POINTS_BASE64)?;
        let positions =
            base64_triples(string(positions(fields)?, "positions")?, "positions", |t| {
                t.map(f64::from)
            })?;
        let colors = match fields.get("colors").filter(|v| !v.is_null()) {
            Some(colors) => Some(base64_triples(string(colors, "colors")?, "colors", |t| t)?),
            None => None,
        };
        Self::checked(positions, colors)
    }

    /// Decodes a `pcd::base64` value: a string holding a base64-encoded
    /// PCD file. See [`PointCloud::from_pcd`].
    pub fn from_pcd_base64(value: &Value) -> Result<Self, DecodeError> {
        let bytes = FORGIVING
            .decode(&*without_whitespace(string(value, PCD_BASE64)?))
            .map_err(|e| DecodeError::InvalidBase64 {
                at: PCD_BASE64.to_owned(),
                reason: e.to_string(),
            })?;
        Self::from_pcd(&bytes)
    }

    /// Reads a PCD v0.7 file the way three.js `PCDLoader` r171 does.
    ///
    /// - `DATA ascii`, `binary` (row-major), and `binary_compressed` (LZF,
    ///   column-major) are read. Other `DATA` values are
    ///   [`DecodeError::UnsupportedPcd`].
    /// - `x`, `y`, `z` are required. In binary data they must be `SIZE 4`,
    ///   `TYPE F`, `COUNT 1`, the only layout the loader reads correctly.
    /// - `rgb` is optional, packed as `0x00RRGGBB` in four bytes, and
    ///   converted from sRGB to linear. In ASCII data a `TYPE F` value is
    ///   reinterpreted bit for bit, as PCL writes it.
    /// - Other fields (normals, intensity, label, …) are skipped.
    /// - Multi-byte binary values are little-endian.
    pub fn from_pcd(bytes: &[u8]) -> Result<Self, DecodeError> {
        let header = PcdHeader::parse(bytes)?;
        let body = &bytes[header.len..];
        match header.data.as_str() {
            "ascii" => read_pcd_ascii(&header, body),
            "binary" => read_pcd_binary(&header, body, Layout::Rows),
            "binary_compressed" => {
                let decompressed = read_pcd_compressed(body)?;
                read_pcd_binary(&header, &decompressed, Layout::Columns)
            }
            other => Err(unsupported(format!("DATA {other}"))),
        }
    }

    fn checked(positions: Vec<Vec3>, colors: Option<Vec<[f32; 3]>>) -> Result<Self, DecodeError> {
        if let Some(colors) = &colors {
            if colors.len() != positions.len() {
                return Err(DecodeError::LengthMismatch {
                    field: "colors",
                    len: colors.len(),
                    expected: positions.len(),
                });
            }
        }
        Ok(Self { positions, colors })
    }
}

fn positions(fields: &serde_json::Map<String, Value>) -> Result<&Value, DecodeError> {
    fields
        .get("positions")
        .filter(|v| !v.is_null())
        .ok_or(DecodeError::MissingField { field: "positions" })
}

fn string<'v>(value: &'v Value, at: &str) -> Result<&'v str, DecodeError> {
    value.as_str().ok_or_else(|| DecodeError::WrongShape {
        at: at.to_owned(),
        expected: "base64 string",
    })
}

/// Standard alphabet, padding optional, trailing bits ignored: what browsers'
/// `atob` (WHATWG forgiving-base64) accepts once whitespace is removed.
const FORGIVING: GeneralPurpose = GeneralPurpose::new(
    &alphabet::STANDARD,
    GeneralPurposeConfig::new()
        .with_decode_padding_mode(DecodePaddingMode::Indifferent)
        .with_decode_allow_trailing_bits(true),
);

/// Base64 symbols per chunk: a multiple of 16, so each chunk decodes to whole
/// 12-byte triples.
const CHUNK_SYMBOLS: usize = 16 * 1024;
const CHUNK_BYTES: usize = CHUNK_SYMBOLS / 4 * 3;

fn without_whitespace(text: &str) -> Cow<'_, [u8]> {
    let bytes = text.as_bytes();
    if bytes.iter().any(u8::is_ascii_whitespace) {
        Cow::Owned(
            bytes
                .iter()
                .copied()
                .filter(|b| !b.is_ascii_whitespace())
                .collect(),
        )
    } else {
        Cow::Borrowed(bytes)
    }
}

/// Decodes base64 `f32` triples chunk by chunk into a vector allocated once.
fn base64_triples<T>(
    text: &str,
    at: &str,
    convert: impl Fn([f32; 3]) -> T,
) -> Result<Vec<T>, DecodeError> {
    let invalid = |reason: String| DecodeError::InvalidBase64 {
        at: at.to_owned(),
        reason,
    };
    let input = without_whitespace(text);
    let mut symbols = input.len();
    for _ in 0..2 {
        if symbols > 0 && input[symbols - 1] == b'=' {
            symbols -= 1;
        }
    }
    let bytes = symbols / 4 * 3
        + match symbols % 4 {
            0 => 0,
            2 => 1,
            3 => 2,
            _ => {
                return Err(invalid(format!(
                    "{symbols} symbols cannot encode whole bytes"
                )))
            }
        };
    let byte_length = || DecodeError::ByteLength {
        at: at.to_owned(),
        bytes,
        element_size: 12,
    };
    if bytes % 12 != 0 {
        return Err(byte_length());
    }

    let mut out = Vec::with_capacity(bytes / 12);
    let mut buffer = [0u8; CHUNK_BYTES];
    for chunk in input.chunks(CHUNK_SYMBOLS) {
        let written = FORGIVING
            .decode_slice(chunk, &mut buffer)
            .map_err(|e| match e {
                DecodeSliceError::DecodeError(e) => invalid(e.to_string()),
                DecodeSliceError::OutputSliceTooSmall => invalid("padding inside the data".into()),
            })?;
        if written % 12 != 0 {
            return Err(byte_length());
        }
        out.extend(
            buffer[..written]
                .chunks_exact(12)
                .map(|t| convert([f32_le(&t[0..4]), f32_le(&t[4..8]), f32_le(&t[8..12])])),
        );
    }
    Ok(out)
}

fn f32_le(b: &[u8]) -> f32 {
    f32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

fn u32_le(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}

/// sRGB to linear, with the constants of three.js `SRGBToLinear`.
fn srgb_to_linear(c: f64) -> f64 {
    if c < 0.04045 {
        c * 0.0773993808
    } else {
        (c * 0.9478672986 + 0.0521327014).powf(2.4)
    }
}

/// Linear colour of a PCD `rgb` value packed as `0x00RRGGBB`.
fn packed_rgb(packed: u32) -> [f32; 3] {
    let channel = |shift: u32| srgb_to_linear(f64::from((packed >> shift) & 0xff) / 255.0) as f32;
    [channel(16), channel(8), channel(0)]
}

fn invalid(reason: impl Into<String>) -> DecodeError {
    DecodeError::InvalidPcd {
        reason: reason.into(),
    }
}

fn unsupported(reason: impl Into<String>) -> DecodeError {
    DecodeError::UnsupportedPcd {
        reason: reason.into(),
    }
}

/// The parts of a PCD header the reader uses.
#[derive(Debug)]
struct PcdHeader {
    fields: Vec<String>,
    size: Option<Vec<usize>>,
    kind: Option<Vec<String>>,
    count: Vec<usize>,
    points: Option<usize>,
    data: String,
    /// Header length in bytes, including the newline after `DATA`.
    len: usize,
}

impl PcdHeader {
    fn parse(bytes: &[u8]) -> Result<Self, DecodeError> {
        let mut fields = None;
        let mut size = None;
        let mut kind = None;
        let mut count = None;
        let mut width = None;
        let mut height = None;
        let mut points = None;
        let mut pos = 0;
        loop {
            if pos >= bytes.len() {
                return Err(invalid("no DATA line"));
            }
            let end = bytes[pos..]
                .iter()
                .position(|&b| b == b'\n')
                .map_or(bytes.len(), |i| pos + i);
            let line = std::str::from_utf8(&bytes[pos..end])
                .map_err(|_| invalid("header line is not UTF-8"))?;
            let next = (end + 1).min(bytes.len());
            let line = line.split('#').next().unwrap_or_default();
            let mut tokens = line.split_ascii_whitespace();
            let Some(keyword) = tokens.next() else {
                pos = next;
                continue;
            };
            let values = || tokens.clone().map(str::to_owned).collect::<Vec<_>>();
            match keyword.to_ascii_uppercase().as_str() {
                "FIELDS" => fields = Some(values()),
                "SIZE" => size = Some(numbers("SIZE", tokens.clone())?),
                "TYPE" => kind = Some(values()),
                "COUNT" => count = Some(numbers("COUNT", tokens.clone())?),
                "WIDTH" => width = Some(number("WIDTH", tokens.clone())?),
                "HEIGHT" => height = Some(number("HEIGHT", tokens.clone())?),
                "POINTS" => points = Some(number("POINTS", tokens.clone())?),
                "DATA" => {
                    let data = tokens.next().ok_or_else(|| invalid("empty DATA line"))?;
                    let fields: Vec<String> = fields.ok_or_else(|| invalid("no FIELDS line"))?;
                    let count = count.unwrap_or_else(|| vec![1; fields.len()]);
                    for (name, len) in [
                        ("SIZE", size.as_ref().map(Vec::len)),
                        ("TYPE", kind.as_ref().map(Vec::len)),
                        ("COUNT", Some(count.len())),
                    ] {
                        if let Some(len) = len.filter(|&len| len != fields.len()) {
                            return Err(invalid(format!(
                                "{name} has {len} entries for {} fields",
                                fields.len()
                            )));
                        }
                    }
                    let points = match (points, width, height) {
                        (Some(points), _, _) => Some(points),
                        (None, Some(w), Some(h)) => Some(
                            usize::checked_mul(w, h)
                                .ok_or_else(|| invalid("WIDTH × HEIGHT overflows"))?,
                        ),
                        _ => None,
                    };
                    return Ok(Self {
                        fields,
                        size,
                        kind,
                        count,
                        points,
                        data: data.to_owned(),
                        len: next,
                    });
                }
                _ => {}
            }
            pos = next;
        }
    }

    fn index(&self, name: &str) -> Option<usize> {
        self.fields.iter().position(|f| f == name)
    }

    fn type_of(&self, field: usize) -> Option<&str> {
        self.kind.as_ref().map(|kind| kind[field].as_str())
    }

    fn xyz(&self) -> Result<[usize; 3], DecodeError> {
        match (self.index("x"), self.index("y"), self.index("z")) {
            (Some(x), Some(y), Some(z)) => Ok([x, y, z]),
            _ => Err(invalid("FIELDS lacks x, y, or z")),
        }
    }
}

fn number<'a>(name: &str, mut tokens: impl Iterator<Item = &'a str>) -> Result<usize, DecodeError> {
    tokens
        .next()
        .and_then(|t| t.parse().ok())
        .ok_or_else(|| invalid(format!("{name} is not a count")))
}

fn numbers<'a>(
    name: &str,
    tokens: impl Iterator<Item = &'a str>,
) -> Result<Vec<usize>, DecodeError> {
    tokens
        .map(|t| {
            t.parse()
                .map_err(|_| invalid(format!("{name} entry {t:?} is not a count")))
        })
        .collect()
}

fn read_pcd_ascii(header: &PcdHeader, body: &[u8]) -> Result<PointCloud, DecodeError> {
    let text = std::str::from_utf8(body).map_err(|_| invalid("ASCII data is not UTF-8"))?;
    // Column of each field: earlier fields take COUNT columns each.
    let column = |field: usize| header.count[..field].iter().sum::<usize>();
    // Coordinates go through f32, as the loader's Float32Array does, unless
    // the header declares them as 8-byte values.
    let [x, y, z] = header.xyz()?.map(|f| {
        let double = header.size.as_ref().is_some_and(|size| size[f] == 8);
        (column(f), double)
    });
    let rgb = header
        .index("rgb")
        .map(|i| (column(i), header.type_of(i) == Some("F")));

    let mut positions = Vec::new();
    let mut colors = rgb.map(|_| Vec::new());
    let mut tokens: Vec<&str> = Vec::new();
    for (line_no, line) in text.lines().enumerate() {
        tokens.clear();
        tokens.extend(line.split_ascii_whitespace());
        if tokens.is_empty() {
            continue;
        }
        let value = |col: usize| -> Result<f64, DecodeError> {
            tokens
                .get(col)
                .and_then(|t| t.parse::<f64>().ok())
                .ok_or_else(|| {
                    invalid(format!(
                        "data line {} column {} is not a number",
                        line_no + 1,
                        col + 1
                    ))
                })
        };
        let coordinate = |(col, double): (usize, bool)| {
            value(col).map(|v| if double { v } else { f64::from(v as f32) })
        };
        positions.push([coordinate(x)?, coordinate(y)?, coordinate(z)?]);
        if let (Some((col, is_float)), Some(colors)) = (rgb, colors.as_mut()) {
            let v = value(col)?;
            let packed = if is_float {
                (v as f32).to_bits()
            } else {
                js_to_uint32(v)
            };
            colors.push(packed_rgb(packed));
        }
    }
    Ok(PointCloud { positions, colors })
}

/// ECMAScript ToUint32, which `>>` and `&` apply to the parsed number.
fn js_to_uint32(v: f64) -> u32 {
    if v.is_finite() {
        v.trunc().rem_euclid(4_294_967_296.0) as u32
    } else {
        0
    }
}

#[derive(Clone, Copy)]
enum Layout {
    /// `binary`: one record per point.
    Rows,
    /// `binary_compressed`: all values of a field, then the next field.
    Columns,
}

fn read_pcd_binary(
    header: &PcdHeader,
    data: &[u8],
    layout: Layout,
) -> Result<PointCloud, DecodeError> {
    let size = header
        .size
        .as_ref()
        .ok_or_else(|| invalid("binary data without SIZE"))?;
    let points = header
        .points
        .ok_or_else(|| invalid("binary data without POINTS or WIDTH and HEIGHT"))?;
    let xyz = header.xyz()?;
    for (axis, &field) in ["x", "y", "z"].iter().zip(&xyz) {
        let ty = header.type_of(field);
        if size[field] != 4 || ty != Some("F") || header.count[field] != 1 {
            return Err(unsupported(format!(
                "{axis} is SIZE {} TYPE {} COUNT {}; only 4-byte floats are read",
                size[field],
                ty.unwrap_or("?"),
                header.count[field]
            )));
        }
    }
    let rgb = header.index("rgb");
    if let Some(field) = rgb {
        if size[field] != 4 || header.count[field] != 1 {
            return Err(unsupported("rgb must be one packed 4-byte value"));
        }
    }

    let mut offsets = Vec::with_capacity(size.len());
    let mut row = 0usize;
    for (&s, &c) in size.iter().zip(&header.count) {
        offsets.push(row);
        row = s
            .checked_mul(c)
            .and_then(|n| row.checked_add(n))
            .ok_or_else(|| invalid("record size overflows"))?;
    }
    let needed = points
        .checked_mul(row)
        .ok_or_else(|| invalid("POINTS × record size overflows"))?;
    if data.len() < needed {
        return Err(invalid(format!(
            "{points} points need {needed} data bytes, found {}",
            data.len()
        )));
    }

    // Byte position of field `f` of point `i`.
    let at = |f: usize, i: usize| match layout {
        Layout::Rows => i * row + offsets[f],
        Layout::Columns => points * offsets[f] + i * size[f],
    };
    let positions = (0..points)
        .map(|i| xyz.map(|f| f64::from(f32_le(&data[at(f, i)..]))))
        .collect();
    let colors = rgb.map(|f| {
        (0..points)
            .map(|i| packed_rgb(u32_le(&data[at(f, i)..])))
            .collect()
    });
    Ok(PointCloud { positions, colors })
}

fn read_pcd_compressed(body: &[u8]) -> Result<Vec<u8>, DecodeError> {
    if body.len() < 8 {
        return Err(invalid("binary_compressed data lacks its size words"));
    }
    let compressed = u32_le(&body[0..4]) as usize;
    let decompressed = u32_le(&body[4..8]) as usize;
    let input = body
        .get(8..8usize.saturating_add(compressed))
        .ok_or_else(|| {
            invalid(format!(
                "{compressed} compressed bytes announced, {} present",
                body.len() - 8
            ))
        })?;
    decompress_lzf(input, decompressed)
}

/// Longest LZF expansion: a 3-byte back reference yields up to 264 bytes.
const LZF_MAX_RATIO: usize = 88;

/// LZF decompression as in three.js `PCDLoader`, with the same checks, plus
/// a bound on the announced size so a short input cannot force a huge
/// allocation.
fn decompress_lzf(input: &[u8], out_len: usize) -> Result<Vec<u8>, DecodeError> {
    let corrupt = || invalid("corrupt LZF data");
    if out_len > input.len().saturating_mul(LZF_MAX_RATIO) {
        return Err(invalid(format!(
            "{} LZF bytes cannot expand to {out_len}",
            input.len()
        )));
    }
    let mut out = Vec::with_capacity(out_len);
    let mut i = 0;
    while i < input.len() {
        let ctrl = usize::from(input[i]);
        i += 1;
        if ctrl < 32 {
            let n = ctrl + 1;
            if out.len() + n > out_len || i + n > input.len() {
                return Err(corrupt());
            }
            out.extend_from_slice(&input[i..i + n]);
            i += n;
        } else {
            let mut len = ctrl >> 5;
            if i >= input.len() {
                return Err(corrupt());
            }
            if len == 7 {
                len += usize::from(input[i]);
                i += 1;
                if i >= input.len() {
                    return Err(corrupt());
                }
            }
            let back = ((ctrl & 0x1f) << 8) + usize::from(input[i]) + 1;
            i += 1;
            if out.len() + len + 2 > out_len || back > out.len() {
                return Err(corrupt());
            }
            let start = out.len() - back;
            // Overlapping copies repeat bytes, so copy one at a time.
            for k in start..start + len + 2 {
                out.push(out[k]);
            }
        }
    }
    if out.len() != out_len {
        return Err(invalid(format!(
            "LZF data expands to {} bytes, header says {out_len}",
            out.len()
        )));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lzf_back_reference_repeats_bytes() {
        // Literal "ab", then copy 4 bytes from 2 back: "ababab".
        let input = [1, b'a', b'b', (2 << 5), 1];
        assert_eq!(decompress_lzf(&input, 6).unwrap(), b"ababab");
    }

    #[test]
    fn lzf_rejects_references_before_start_and_overlong_output() {
        assert!(matches!(
            decompress_lzf(&[(1 << 5), 0], 3),
            Err(DecodeError::InvalidPcd { .. })
        ));
        assert!(decompress_lzf(&[0, b'a'], 2).is_err());
        assert!(decompress_lzf(&[0], 10_000).is_err());
    }

    #[test]
    fn srgb_endpoints_stay_put() {
        assert_eq!(packed_rgb(0x00ff_0000), [1.0, 0.0, 0.0]);
        assert_eq!(packed_rgb(0x0000_ff00), [0.0, 1.0, 0.0]);
        assert_eq!(packed_rgb(0x0000_00ff), [0.0, 0.0, 1.0]);
        // Mid grey 0x80 is about 0.216 in linear light.
        let [r, _, _] = packed_rgb(0x0080_8080);
        assert!((r - 0.2158605).abs() < 1e-6, "{r}");
    }

    #[test]
    fn to_uint32_wraps_like_javascript() {
        assert_eq!(js_to_uint32(16_711_680.0), 0x00ff_0000);
        assert_eq!(js_to_uint32(-1.0), u32::MAX);
        assert_eq!(js_to_uint32(f64::NAN), 0);
    }
}
