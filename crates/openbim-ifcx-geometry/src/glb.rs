//! Binary glTF 2.0 (GLB) export of a [`RenderScene`].
//!
//! [`to_glb`] writes one self-contained `.glb` file that any glTF viewer
//! opens. The JSON chunk is built with `serde_json`; no glTF library is
//! used.
//!
//! # Layout
//!
//! - One root node, named `IFCX`, holds the scene's [`origin`] as its
//!   translation and, by default, the rotation from IFCX's Z-up to glTF's
//!   Y-up axes. Its `extras` record the origin in the file's coordinates
//!   (`ifcxOrigin`) and the up axis written (`ifcxUpAxis`).
//! - Each [`Instance`] becomes one child node of the root, named by the
//!   instance path, with the instance's render matrix and `extras.ifcxNode`
//!   set to [`Instance::node`]. A node with several kinds of geometry gives
//!   one glTF node per kind, all with the same name.
//! - Each pair of geometry buffer and material becomes one glTF mesh, which
//!   every instance of that pair references. Each buffer is written once, so
//!   instances share vertex data even across materials.
//! - Meshes are `TRIANGLES` with `POSITION`, `NORMAL`, and `u32` indices;
//!   lines are `LINES` with indices; points are `POINTS` with `COLOR_0` when
//!   they have colours. `POSITION` accessors carry `min` and `max`.
//! - Materials used by an instance map onto `pbrMetallicRoughness`: a
//!   [`BasicMaterial`](crate::BasicMaterial) is its colour with opacity as
//!   alpha, metallic 0, roughness 1, and `BLEND` when not opaque; a
//!   [`GltfMaterial`] keeps its factors, `alphaMode`, `alphaCutoff`, and
//!   `doubleSided`. Texture references are not written, since the scene does
//!   not load them. Factors and colours are clamped to `0..=1`, as glTF
//!   requires, and `alphaCutoff` to a finite number of at least 0 (`0.5`
//!   for NaN). With the scene's own finite coordinates (see
//!   [`SceneWarningKind::OutOfRange`](crate::SceneWarningKind::OutOfRange)),
//!   every number in the JSON chunk is finite, so it never holds `null`.
//! - Every buffer view starts on a 4-byte boundary, and both chunks are
//!   padded to 4 bytes, the JSON chunk with spaces.
//!
//! [`origin`]: RenderScene::origin

use std::collections::HashMap;
use std::fmt;

use serde_json::{json, Map, Value};

use crate::presentation::{AlphaMode, GltfMaterial, Material};
use crate::scene::{GeometryRef, Instance, RenderScene};

/// Options for [`to_glb`].
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct GlbOptions {
    /// Rotate the root from IFCX's Z-up to glTF's Y-up axes, so the model
    /// stands upright in glTF viewers. Default `true`.
    pub y_up: bool,
    /// Put the scene origin on the root node's translation, so the file
    /// keeps the model's coordinates. With `false` the model sits around
    /// the glTF origin and the origin is only recorded in `extras`. Default
    /// `true`.
    pub origin_on_root: bool,
}

impl Default for GlbOptions {
    fn default() -> Self {
        Self {
            y_up: true,
            origin_on_root: true,
        }
    }
}

impl GlbOptions {
    /// Sets [`y_up`](Self::y_up).
    pub fn with_y_up(mut self, y_up: bool) -> Self {
        self.y_up = y_up;
        self
    }

    /// Sets [`origin_on_root`](Self::origin_on_root).
    pub fn with_origin_on_root(mut self, origin_on_root: bool) -> Self {
        self.origin_on_root = origin_on_root;
        self
    }
}

/// Why a scene could not be written as GLB.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum GlbError {
    /// The file would exceed the 4 GiB that GLB's 32-bit lengths allow.
    TooLarge { bytes: u64 },
}

impl fmt::Display for GlbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { bytes } => {
                write!(f, "GLB would be {bytes} bytes, more than 4 GiB")
            }
        }
    }
}

impl std::error::Error for GlbError {}

const ARRAY_BUFFER: u32 = 34962;
const ELEMENT_ARRAY_BUFFER: u32 = 34963;
const FLOAT: u32 = 5126;
const UNSIGNED_INT: u32 = 5125;
const POINTS: u32 = 0;
const LINES: u32 = 1;
const TRIANGLES: u32 = 4;

/// -90° about X: IFCX `(x, y, z)` to glTF `(x, z, -y)`.
const Z_UP_TO_Y_UP: [f64; 4] = [
    -std::f64::consts::FRAC_1_SQRT_2,
    0.0,
    0.0,
    std::f64::consts::FRAC_1_SQRT_2,
];

/// Writes `scene` as a binary glTF 2.0 file.
///
/// ```
/// use openbim_ifcx::{compose, flatten, IfcxNode};
/// use openbim_ifcx_geometry::glb::{to_glb, GlbOptions};
/// use openbim_ifcx_geometry::scene::{RenderScene, SceneOptions};
///
/// let nodes: Vec<IfcxNode> = serde_json::from_str(r#"[
///     {"path": "panel", "attributes": {"usd::usdgeom::mesh": {
///         "points": [[0, 0, 0], [1, 0, 0], [0, 1, 0]], "faceVertexIndices": [0, 1, 2]}}}
/// ]"#)?;
/// let scene = RenderScene::from_composition(&compose(&flatten(&nodes))?, &SceneOptions::default());
/// let glb = to_glb(&scene, &GlbOptions::default())?;
/// assert_eq!(&glb[..4], b"glTF");
/// assert_eq!(glb.len() % 4, 0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn to_glb(scene: &RenderScene, options: &GlbOptions) -> Result<Vec<u8>, GlbError> {
    let mut writer = Writer::default();
    let json = writer.document(scene, options);
    let mut json = serde_json::to_vec(&json).expect("JSON values always serialize");
    pad(&mut json, b' ');
    let bin = writer.bin;

    let bin_chunk = if bin.is_empty() {
        0
    } else {
        8 + bin.len() as u64
    };
    let total = 12 + 8 + json.len() as u64 + bin_chunk;
    let total = u32::try_from(total).map_err(|_| GlbError::TooLarge { bytes: total })?;

    let mut out = Vec::with_capacity(total as usize);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&total.to_le_bytes());
    out.extend_from_slice(&(json.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json);
    if !bin.is_empty() {
        out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        out.extend_from_slice(b"BIN\0");
        out.extend_from_slice(&bin);
    }
    Ok(out)
}

fn pad(bytes: &mut Vec<u8>, with: u8) {
    while bytes.len() % 4 != 0 {
        bytes.push(with);
    }
}

/// An `f32` as the JSON number of its exact value, so readers that parse
/// `f64` and round to `f32` get the same bits back.
fn num(v: f32) -> Value {
    json!(f64::from(v))
}

fn clamp01(v: f32) -> f32 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(0.0, 1.0)
    }
}

/// Accessor indices of one written buffer.
#[derive(Clone, Copy)]
struct Written {
    position: usize,
    normal: Option<usize>,
    color: Option<usize>,
    indices: Option<usize>,
    mode: u32,
}

#[derive(Default)]
struct Writer {
    bin: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
}

impl Writer {
    fn view(&mut self, bytes: impl IntoIterator<Item = [u8; 4]>, target: u32) -> usize {
        pad(&mut self.bin, 0);
        let offset = self.bin.len();
        for b in bytes {
            self.bin.extend_from_slice(&b);
        }
        self.views.push(json!({
            "buffer": 0,
            "byteOffset": offset,
            "byteLength": self.bin.len() - offset,
            "target": target,
        }));
        self.views.len() - 1
    }

    fn vec3(&mut self, values: &[[f32; 3]], min_max: bool) -> usize {
        let view = self.view(
            values.iter().flatten().map(|v| v.to_le_bytes()),
            ARRAY_BUFFER,
        );
        let mut accessor = json!({
            "bufferView": view,
            "componentType": FLOAT,
            "count": values.len(),
            "type": "VEC3",
        });
        if min_max {
            let mut min = [f32::INFINITY; 3];
            let mut max = [f32::NEG_INFINITY; 3];
            for v in values {
                for k in 0..3 {
                    min[k] = min[k].min(v[k]);
                    max[k] = max[k].max(v[k]);
                }
            }
            accessor["min"] = json!(min.map(num));
            accessor["max"] = json!(max.map(num));
        }
        self.accessors.push(accessor);
        self.accessors.len() - 1
    }

    fn indices(&mut self, indices: &[u32]) -> usize {
        let view = self.view(
            indices.iter().map(|i| i.to_le_bytes()),
            ELEMENT_ARRAY_BUFFER,
        );
        self.accessors.push(json!({
            "bufferView": view,
            "componentType": UNSIGNED_INT,
            "count": indices.len(),
            "type": "SCALAR",
        }));
        self.accessors.len() - 1
    }

    fn geometry(&mut self, scene: &RenderScene, geometry: GeometryRef) -> Written {
        match geometry {
            GeometryRef::Mesh(i) => {
                let m = &scene.meshes[i];
                Written {
                    position: self.vec3(&m.positions, true),
                    normal: Some(self.vec3(&m.normals, false)),
                    color: None,
                    indices: Some(self.indices(&m.indices)),
                    mode: TRIANGLES,
                }
            }
            GeometryRef::Lines(i) => {
                let l = &scene.lines[i];
                Written {
                    position: self.vec3(&l.positions, true),
                    normal: None,
                    color: None,
                    indices: Some(self.indices(&l.indices)),
                    mode: LINES,
                }
            }
            GeometryRef::Points(i) => {
                let p = &scene.points[i];
                let colors = p.colors.as_ref().map(|colors| {
                    let clamped: Vec<[f32; 3]> = colors.iter().map(|c| c.map(clamp01)).collect();
                    self.vec3(&clamped, false)
                });
                Written {
                    position: self.vec3(&p.positions, true),
                    normal: None,
                    color: colors,
                    indices: None,
                    mode: POINTS,
                }
            }
        }
    }

    fn document(&mut self, scene: &RenderScene, options: &GlbOptions) -> Value {
        let mut written: HashMap<GeometryRef, Written> = HashMap::new();
        let mut meshes: Vec<Value> = Vec::new();
        let mut mesh_ids: HashMap<(GeometryRef, usize), usize> = HashMap::new();
        // Only materials some instance uses, in order of first use.
        let mut materials: Vec<Value> = Vec::new();
        let mut material_ids: HashMap<usize, usize> = HashMap::new();
        let mut nodes: Vec<Value> = vec![Value::Null];
        for instance in &scene.instances {
            let w = *written
                .entry(instance.geometry)
                .or_insert_with(|| self.geometry(scene, instance.geometry));
            let mesh = *mesh_ids
                .entry((instance.geometry, instance.material))
                .or_insert_with(|| {
                    let material_id = *material_ids.entry(instance.material).or_insert_with(|| {
                        materials.push(material(&scene.materials[instance.material]));
                        materials.len() - 1
                    });
                    meshes.push(mesh(&w, material_id));
                    meshes.len() - 1
                });
            nodes.push(instance_node(instance, mesh));
        }

        let mut root = Map::new();
        root.insert("name".into(), json!("IFCX"));
        if nodes.len() > 1 {
            root.insert(
                "children".into(),
                json!((1..nodes.len()).collect::<Vec<_>>()),
            );
        }
        let o = scene.origin;
        let translation = match (options.origin_on_root, options.y_up) {
            (false, _) => [0.0; 3],
            (true, false) => o,
            (true, true) => [o[0], o[2], -o[1]],
        };
        if translation != [0.0; 3] {
            root.insert("translation".into(), json!(translation));
        }
        if options.y_up {
            root.insert("rotation".into(), json!(Z_UP_TO_Y_UP));
        }
        root.insert(
            "extras".into(),
            json!({"ifcxOrigin": o, "ifcxUpAxis": if options.y_up { "Y" } else { "Z" }}),
        );
        nodes[0] = Value::Object(root);

        let mut document = json!({
            "asset": {
                "version": "2.0",
                "generator": concat!("openbim-ifcx-geometry ", env!("CARGO_PKG_VERSION")),
            },
            "scene": 0,
            "scenes": [{"name": "IFCX", "nodes": [0]}],
            "nodes": nodes,
        });
        if !meshes.is_empty() {
            document["meshes"] = Value::Array(meshes);
            document["materials"] = Value::Array(materials);
            document["accessors"] = Value::Array(std::mem::take(&mut self.accessors));
            document["bufferViews"] = Value::Array(std::mem::take(&mut self.views));
            pad(&mut self.bin, 0);
            document["buffers"] = json!([{"byteLength": self.bin.len()}]);
        }
        document
    }
}

fn mesh(w: &Written, material: usize) -> Value {
    let mut attributes = Map::new();
    attributes.insert("POSITION".into(), json!(w.position));
    if let Some(n) = w.normal {
        attributes.insert("NORMAL".into(), json!(n));
    }
    if let Some(c) = w.color {
        attributes.insert("COLOR_0".into(), json!(c));
    }
    let mut primitive = json!({"attributes": attributes, "material": material, "mode": w.mode});
    if let Some(i) = w.indices {
        primitive["indices"] = json!(i);
    }
    json!({"primitives": [primitive]})
}

fn instance_node(instance: &Instance, mesh: usize) -> Value {
    let mut node = json!({
        "name": instance.path,
        "mesh": mesh,
        "extras": {"ifcxNode": instance.node},
    });
    const IDENTITY: [f32; 16] = [
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ];
    if instance.matrix != IDENTITY {
        node["matrix"] = json!(instance.matrix.map(num));
    }
    node
}

fn material(material: &Material) -> Value {
    match material {
        Material::Basic(basic) => {
            let [r, g, b] = basic.color.map(clamp01);
            let a = clamp01(basic.opacity);
            let mut m = json!({
                "pbrMetallicRoughness": {
                    "baseColorFactor": [num(r), num(g), num(b), num(a)],
                    "metallicFactor": 0.0,
                    "roughnessFactor": 1.0,
                },
            });
            if a < 1.0 {
                m["alphaMode"] = json!("BLEND");
            }
            m
        }
        Material::Pbr(pbr) => pbr_material(pbr),
    }
}

fn pbr_material(pbr: &GltfMaterial) -> Value {
    let mut m = json!({
        "pbrMetallicRoughness": {
            "baseColorFactor": pbr.base_color_factor.map(clamp01).map(num),
            "metallicFactor": num(clamp01(pbr.metallic_factor)),
            "roughnessFactor": num(clamp01(pbr.roughness_factor)),
        },
    });
    if pbr.emissive_factor != [0.0; 3] {
        m["emissiveFactor"] = json!(pbr.emissive_factor.map(clamp01).map(num));
    }
    match pbr.alpha_mode {
        AlphaMode::Opaque => {}
        AlphaMode::Blend => m["alphaMode"] = json!("BLEND"),
        AlphaMode::Mask => {
            m["alphaMode"] = json!("MASK");
            // glTF wants a finite number >= 0; NaN means the default.
            let cutoff = if pbr.alpha_cutoff.is_nan() {
                0.5
            } else {
                pbr.alpha_cutoff.clamp(0.0, f32::MAX)
            };
            m["alphaCutoff"] = num(cutoff);
        }
    }
    if pbr.double_sided {
        m["doubleSided"] = json!(true);
    }
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chunks(glb: &[u8]) -> (Value, &[u8]) {
        let u32_at = |i: usize| u32::from_le_bytes(glb[i..i + 4].try_into().unwrap()) as usize;
        assert_eq!(&glb[0..4], b"glTF");
        assert_eq!(u32_at(4), 2);
        assert_eq!(u32_at(8), glb.len());
        let json_len = u32_at(12);
        assert_eq!(&glb[16..20], b"JSON");
        let json = serde_json::from_slice(&glb[20..20 + json_len]).unwrap();
        let rest = &glb[20 + json_len..];
        if rest.is_empty() {
            return (json, rest);
        }
        assert_eq!(&rest[4..8], b"BIN\0");
        let bin_len = u32::from_le_bytes(rest[0..4].try_into().unwrap()) as usize;
        assert_eq!(rest.len(), 8 + bin_len);
        (json, &rest[8..])
    }

    #[test]
    fn empty_scene_has_only_a_root() {
        let glb = to_glb(&RenderScene::default(), &GlbOptions::default()).unwrap();
        assert_eq!(glb.len() % 4, 0);
        let (json, bin) = chunks(&glb);
        assert!(bin.is_empty());
        assert_eq!(json["nodes"].as_array().unwrap().len(), 1);
        assert!(json.get("buffers").is_none());
        assert!(json["nodes"][0].get("translation").is_none());
    }

    #[test]
    fn clamps_out_of_range_factors() {
        let m = material(&Material::Basic(crate::BasicMaterial {
            color: [1.5, -0.2, 0.5],
            opacity: 0.25,
        }));
        assert_eq!(
            m["pbrMetallicRoughness"]["baseColorFactor"],
            json!([1.0, 0.0, 0.5, 0.25])
        );
        assert_eq!(m["alphaMode"], "BLEND");
    }

    /// Found by fuzzing (#41, target `scene_glb`): an infinite cutoff was
    /// written as JSON `null`.
    #[test]
    fn alpha_cutoff_is_always_a_finite_number() {
        for (cutoff, written) in [
            (f32::INFINITY, f64::from(f32::MAX)),
            (f32::NEG_INFINITY, 0.0),
            (f32::NAN, 0.5),
            (0.25, 0.25),
        ] {
            let m = pbr_material(&GltfMaterial {
                alpha_mode: AlphaMode::Mask,
                alpha_cutoff: cutoff,
                ..GltfMaterial::default()
            });
            assert_eq!(m["alphaCutoff"].as_f64(), Some(written), "{cutoff}");
        }
    }

    #[test]
    fn exact_f32_numbers() {
        assert_eq!(num(0.1).as_f64().unwrap() as f32, 0.1f32);
        assert_eq!(num(0.1).as_f64().unwrap(), f64::from(0.1f32));
    }
}
