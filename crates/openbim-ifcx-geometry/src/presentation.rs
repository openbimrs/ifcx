//! Visibility, colour, opacity, and glTF PBR materials (`ifcx_alpha`).
//!
//! [`NodePresentation::from_attributes`] decodes one node's own presentation
//! attributes. The functions [`is_visible`], [`resolve_basic_material`], and
//! [`resolve_mesh_material`] apply the precedence of the upstream reference
//! viewer (`src/viewer/render.ts` at buildingSMART/IFC5-development
//! `1a63082`) to a node and its ancestors. Walking the hierarchy is left to
//! the caller, the render scene of the tracking issue.
//!
//! | Attribute | Value |
//! | --- | --- |
//! | `usd::usdgeom::visibility` | `{"visibility": "inherited" \| "invisible"}` |
//! | `bsi::ifc::presentation::diffuseColor` | `[r, g, b]`, linear, `0..=1` |
//! | `bsi::ifc::presentation::opacity` | number, `0..=1` |
//! | `gltf::material` | glTF 2.0 material subset, see [`GltfMaterial`] |
//!
//! # Material bindings
//!
//! In `ifcx_alpha` a node binds a material by inheriting from the material
//! node (`"inherits": {"material": "<path>"}`); composition then gives the node
//! the material's `bsi::ifc::presentation::*` and `gltf::material` attributes.
//! The pre-alpha `usd::usdshade::material::outputs::surface` connections were
//! converted to that form upstream: no upstream example uses them, and the
//! reference viewer only lists the name for an outline icon. They are
//! therefore not read here.
//!
//! # Precedence
//!
//! Given a node followed by its ancestors, nearest first:
//!
//! 1. A node is drawn only if neither it nor any ancestor is `invisible`.
//! 2. A mesh uses the `gltf::material` of the nearest node that has one, with
//!    glTF defaults for missing factors, and ignores the
//!    `bsi::ifc::presentation` attributes.
//! 3. Otherwise, and always for curves, colour comes from the nearest node
//!    with `diffuseColor`, and opacity from that same node only. Without any
//!    `diffuseColor` the colour is [`BasicMaterial::DEFAULT`], grey 0.6 and
//!    opaque.
//!
//! Point clouds use neither; they carry their own per-point colours.
//!
//! Three choices differ from the reference viewer where its behaviour follows
//! from JavaScript truthiness or branch order rather than intent:
//!
//! - Opacity `0` is fully transparent; the viewer skips falsy opacity values
//!   and draws such objects opaque.
//! - `visibility: "inherited"` hides nothing; the viewer also skips the
//!   geometry of any node that merely has the attribute.
//! - A `gltf::material` whose members are all `false` or `0` still selects
//!   PBR; the viewer skips it.

use serde_json::{Map, Value};

use crate::json::{field, object};
use crate::{Attributes, DecodeError};

/// Attribute id of USD visibility.
pub const VISIBILITY: &str = "usd::usdgeom::visibility";
/// Attribute id of the IFC diffuse colour.
pub const DIFFUSE_COLOR: &str = "bsi::ifc::presentation::diffuseColor";
/// Attribute id of the IFC opacity.
pub const OPACITY: &str = "bsi::ifc::presentation::opacity";
/// Attribute id of a glTF PBR material.
pub const GLTF_MATERIAL: &str = "gltf::material";

fn wrong(at: impl Into<String>, expected: &'static str) -> DecodeError {
    DecodeError::WrongShape {
        at: at.into(),
        expected,
    }
}

/// USD visibility token of one node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Visibility {
    /// Visible if the parent is visible (USD's default).
    Inherited,
    /// Hidden, together with every descendant.
    Invisible,
}

impl Visibility {
    /// Decodes a `usd::usdgeom::visibility` value, `{"visibility": "<token>"}`.
    pub fn from_value(value: &Value) -> Result<Self, DecodeError> {
        let token = field(object(value, VISIBILITY)?, "visibility")?
            .as_str()
            .ok_or_else(|| wrong("visibility", "string"))?;
        match token {
            "inherited" => Ok(Self::Inherited),
            "invisible" => Ok(Self::Invisible),
            other => Err(DecodeError::UnknownToken {
                at: "visibility".to_owned(),
                token: other.to_owned(),
                expected: "inherited or invisible",
            }),
        }
    }
}

/// Whether a node is drawn, given whether its parent is (`true` for a root)
/// and the node's own visibility. An `invisible` node hides its subtree; a
/// descendant cannot become visible again.
pub fn is_visible(parent_visible: bool, own: Option<Visibility>) -> bool {
    parent_visible && own != Some(Visibility::Invisible)
}

/// glTF `alphaMode`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AlphaMode {
    #[default]
    Opaque,
    Mask,
    Blend,
}

/// A normal map reference. The texture is recorded, not loaded.
#[derive(Debug, Clone, PartialEq)]
pub struct NormalTexture {
    /// Texture location as written, usually a URI.
    pub texture: String,
    /// glTF `scale`, default 1.
    pub scale: f32,
}

/// An occlusion map reference. The texture is recorded, not loaded.
#[derive(Debug, Clone, PartialEq)]
pub struct OcclusionTexture {
    /// Texture location as written, usually a URI.
    pub texture: String,
    /// glTF `strength`, default 1.
    pub strength: f32,
}

/// A `gltf::material` value with glTF 2.0 defaults filled in.
///
/// Factors are linear, as in glTF. Texture members keep the location string
/// as written; nothing is fetched or decoded. The reference viewer maps only
/// the RGB of `baseColorFactor`, `metallicFactor`, and `roughnessFactor`; the
/// other members are kept for the scene and GLB export.
#[derive(Debug, Clone, PartialEq)]
pub struct GltfMaterial {
    /// `pbrMetallicRoughness.baseColorFactor`, default `[1, 1, 1, 1]`.
    pub base_color_factor: [f32; 4],
    /// `pbrMetallicRoughness.baseColorTexture`.
    pub base_color_texture: Option<String>,
    /// `pbrMetallicRoughness.metallicFactor`, default 1.
    pub metallic_factor: f32,
    /// `pbrMetallicRoughness.roughnessFactor`, default 1.
    pub roughness_factor: f32,
    /// `pbrMetallicRoughness.metallicRoughnessTexture`.
    pub metallic_roughness_texture: Option<String>,
    pub normal_texture: Option<NormalTexture>,
    pub occlusion_texture: Option<OcclusionTexture>,
    pub emissive_texture: Option<String>,
    /// Default `[0, 0, 0]`.
    pub emissive_factor: [f32; 3],
    /// Default [`AlphaMode::Opaque`].
    pub alpha_mode: AlphaMode,
    /// Default 0.5.
    pub alpha_cutoff: f32,
    /// Default `false`.
    pub double_sided: bool,
}

impl Default for GltfMaterial {
    fn default() -> Self {
        Self {
            base_color_factor: [1.0; 4],
            base_color_texture: None,
            metallic_factor: 1.0,
            roughness_factor: 1.0,
            metallic_roughness_texture: None,
            normal_texture: None,
            occlusion_texture: None,
            emissive_texture: None,
            emissive_factor: [0.0; 3],
            alpha_mode: AlphaMode::Opaque,
            alpha_cutoff: 0.5,
            double_sided: false,
        }
    }
}

const GLTF_MEMBERS: [&str; 8] = [
    "pbrMetallicRoughness",
    "normalTexture",
    "occlusionTexture",
    "emissiveTexture",
    "emissiveFactor",
    "alphaMode",
    "alphaCutoff",
    "doubleSided",
];

impl GltfMaterial {
    /// Decodes a `gltf::material` value. Returns `None` when the object sets
    /// none of the members the reference viewer looks for, in which case the
    /// node falls back to `bsi::ifc::presentation`. Unknown members are
    /// ignored.
    pub fn from_value(value: &Value) -> Result<Option<Self>, DecodeError> {
        let object = object(value, GLTF_MATERIAL)?;
        if !GLTF_MEMBERS.iter().any(|m| member(object, m).is_some()) {
            return Ok(None);
        }
        let mut material = Self::default();
        if let Some(pbr) = member(object, "pbrMetallicRoughness") {
            let pbr = pbr
                .as_object()
                .ok_or_else(|| wrong("pbrMetallicRoughness", "object"))?;
            let at = |name: &str| format!("pbrMetallicRoughness.{name}");
            if let Some(v) = member(pbr, "baseColorFactor") {
                material.base_color_factor =
                    reals(v).ok_or_else(|| wrong(at("baseColorFactor"), "array of 4 numbers"))?;
            }
            material.base_color_texture = text(pbr, "baseColorTexture", at)?;
            if let Some(v) = member(pbr, "metallicFactor") {
                material.metallic_factor = real(v, at("metallicFactor"))?;
            }
            if let Some(v) = member(pbr, "roughnessFactor") {
                material.roughness_factor = real(v, at("roughnessFactor"))?;
            }
            material.metallic_roughness_texture = text(pbr, "metallicRoughnessTexture", at)?;
        }
        if let Some((texture, scale)) = texture_info(object, "normalTexture", "scale")? {
            material.normal_texture = Some(NormalTexture { texture, scale });
        }
        if let Some((texture, strength)) = texture_info(object, "occlusionTexture", "strength")? {
            material.occlusion_texture = Some(OcclusionTexture { texture, strength });
        }
        material.emissive_texture = text(object, "emissiveTexture", str::to_owned)?;
        if let Some(v) = member(object, "emissiveFactor") {
            material.emissive_factor =
                reals(v).ok_or_else(|| wrong("emissiveFactor", "array of 3 numbers"))?;
        }
        if let Some(v) = member(object, "alphaMode") {
            material.alpha_mode = match v.as_str() {
                Some("OPAQUE") => AlphaMode::Opaque,
                Some("MASK") => AlphaMode::Mask,
                Some("BLEND") => AlphaMode::Blend,
                _ => {
                    return Err(DecodeError::UnknownToken {
                        at: "alphaMode".to_owned(),
                        token: v.as_str().map_or_else(|| v.to_string(), str::to_owned),
                        expected: "OPAQUE, MASK, or BLEND",
                    })
                }
            };
        }
        if let Some(v) = member(object, "alphaCutoff") {
            material.alpha_cutoff = real(v, "alphaCutoff")?;
        }
        if let Some(v) = member(object, "doubleSided") {
            material.double_sided = v.as_bool().ok_or_else(|| wrong("doubleSided", "boolean"))?;
        }
        Ok(Some(material))
    }
}

fn member<'v>(object: &'v Map<String, Value>, name: &str) -> Option<&'v Value> {
    object.get(name).filter(|v| !v.is_null())
}

fn real(value: &Value, at: impl Into<String>) -> Result<f32, DecodeError> {
    value
        .as_f64()
        .map(|v| v as f32)
        .ok_or_else(|| wrong(at, "number"))
}

fn reals<const N: usize>(value: &Value) -> Option<[f32; N]> {
    let items = value.as_array()?;
    if items.len() != N {
        return None;
    }
    let mut out = [0.0; N];
    for (slot, item) in out.iter_mut().zip(items) {
        *slot = item.as_f64()? as f32;
    }
    Some(out)
}

fn text(
    object: &Map<String, Value>,
    name: &str,
    at: impl Fn(&str) -> String,
) -> Result<Option<String>, DecodeError> {
    member(object, name)
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or_else(|| wrong(at(name), "string"))
        })
        .transpose()
}

fn texture_info(
    object: &Map<String, Value>,
    name: &str,
    factor: &str,
) -> Result<Option<(String, f32)>, DecodeError> {
    let Some(value) = member(object, name) else {
        return Ok(None);
    };
    let info = value.as_object().ok_or_else(|| wrong(name, "object"))?;
    let at = |field: &str| format!("{name}.{field}");
    let texture =
        text(info, "texture", at)?.ok_or(DecodeError::MissingField { field: "texture" })?;
    let factor = match member(info, factor) {
        Some(v) => real(v, at(factor))?,
        None => 1.0,
    };
    Ok(Some((texture, factor)))
}

/// One node's own presentation attributes, before any inheritance.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NodePresentation {
    pub visibility: Option<Visibility>,
    /// Linear RGB in `0..=1`, as written.
    pub diffuse_color: Option<[f32; 3]>,
    pub opacity: Option<f32>,
    pub gltf_material: Option<GltfMaterial>,
}

impl NodePresentation {
    /// Decodes the node's `usd::usdgeom::visibility`,
    /// `bsi::ifc::presentation::diffuseColor` and `opacity`, and
    /// `gltf::material`. Absent or `null` attributes stay `None`.
    pub fn from_attributes<A: Attributes + ?Sized>(attributes: &A) -> Result<Self, DecodeError> {
        let visibility = attributes
            .attribute(VISIBILITY)
            .map(Visibility::from_value)
            .transpose()?;
        let diffuse_color = attributes
            .attribute(DIFFUSE_COLOR)
            .map(|v| reals(v).ok_or_else(|| wrong(DIFFUSE_COLOR, "array of 3 numbers")))
            .transpose()?;
        let opacity = attributes
            .attribute(OPACITY)
            .map(|v| real(v, OPACITY))
            .transpose()?;
        let gltf_material = match attributes.attribute(GLTF_MATERIAL) {
            Some(v) => GltfMaterial::from_value(v)?,
            None => None,
        };
        Ok(Self {
            visibility,
            diffuse_color,
            opacity,
            gltf_material,
        })
    }
}

/// Colour and opacity from `bsi::ifc::presentation`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BasicMaterial {
    /// Linear RGB in `0..=1`.
    pub color: [f32; 3],
    /// `1` is opaque.
    pub opacity: f32,
}

impl BasicMaterial {
    /// What the reference viewer draws without any `diffuseColor`: grey 0.6,
    /// opaque.
    pub const DEFAULT: Self = Self {
        color: [0.6; 3],
        opacity: 1.0,
    };

    /// Whether the material needs blending.
    pub fn is_transparent(&self) -> bool {
        self.opacity < 1.0
    }
}

impl Default for BasicMaterial {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// The material a mesh is drawn with.
#[derive(Debug, Clone, PartialEq)]
pub enum Material {
    /// From `bsi::ifc::presentation`, or the default grey.
    Basic(BasicMaterial),
    /// From `gltf::material`.
    Pbr(GltfMaterial),
}

/// Colour and opacity for a node, from the node and its ancestors given
/// nearest first. The nearest `diffuseColor` wins, and opacity comes from
/// that same node (default 1); an opacity on a node without `diffuseColor` is
/// not used. Used for curves, and for meshes without a glTF material.
pub fn resolve_basic_material<'a, I>(node_then_ancestors: I) -> BasicMaterial
where
    I: IntoIterator<Item = &'a NodePresentation>,
{
    node_then_ancestors
        .into_iter()
        .find_map(|p| {
            p.diffuse_color.map(|color| BasicMaterial {
                color,
                opacity: p.opacity.unwrap_or(1.0),
            })
        })
        .unwrap_or_default()
}

/// The material for a mesh node, from the node and its ancestors given
/// nearest first. The nearest `gltf::material` wins over any
/// `bsi::ifc::presentation` attribute; without one, see
/// [`resolve_basic_material`].
pub fn resolve_mesh_material<'a, I>(node_then_ancestors: I) -> Material
where
    I: IntoIterator<Item = &'a NodePresentation>,
    I::IntoIter: Clone,
{
    let nodes = node_then_ancestors.into_iter();
    match nodes.clone().find_map(|p| p.gltf_material.as_ref()) {
        Some(pbr) => Material::Pbr(pbr.clone()),
        None => Material::Basic(resolve_basic_material(nodes)),
    }
}
