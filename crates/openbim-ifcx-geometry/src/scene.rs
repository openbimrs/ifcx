//! A flat, renderer-neutral render scene over a composed node tree.
//!
//! [`RenderScene::from_composition`] walks the composed tree of
//! [`openbim_ifcx::compose()`] once and returns everything a viewer draws:
//!
//! - [`Instance`]s: one per drawn geometry of a node, with the node's path
//!   (for picking), its world [`Transform`], a [`GeometryRef`] into the shared
//!   buffers, a material index, and world bounds;
//! - shared geometry buffers: [`MeshBuffer`] (triangles with vertex normals),
//!   [`LineBuffer`] (line segments), [`PointBuffer`] (points with optional
//!   colours), all `f32`;
//! - the [`Material`] table;
//! - the scene's `f64` world [`Bounds`] and the render [`origin`](RenderScene::origin);
//! - [`SceneWarning`]s for attribute values that could not be decoded.
//!
//! # Walk
//!
//! Starting at the artificial root, each node is visited once per path that
//! reaches it, parents before children, children in composed order. For each
//! node:
//!
//! 1. An `invisible` node is left out with its whole subtree
//!    ([`is_visible`](crate::is_visible)).
//! 2. Its world transform is its `usd::xformop` applied in its parent's world
//!    transform ([`world_from_parent`]); a node without one inherits it.
//! 3. A mesh is drawn with the nearest `gltf::material`, else the nearest
//!    `diffuseColor` with its own opacity, else grey 0.6 opaque; curves use
//!    the latter two only ([`resolve_mesh_material`](crate::resolve_mesh_material),
//!    [`resolve_basic_material`](crate::resolve_basic_material)).
//! 4. Each of its mesh, curves, and point cloud becomes an instance.
//!
//! Instance paths name the node by child names from the root, as
//! `site/Storey/Wall`, which [`Composition::get`] resolves. [`Instance::node`]
//! is the path of the node that defined it, [`ComposedNode::path`].
//!
//! # Instancing
//!
//! Composition shares sub-trees and attribute values through [`Arc`]: every
//! node that inherits a type, or names the same child, holds the same
//! attribute value. The scene decodes each distinct value once, keyed by its
//! `Arc` pointer, and stores one buffer that all instances reference. Equal
//! values written separately in a file are not merged.
//!
//! # Precision and origin
//!
//! Coordinates stay `f64` until the end. Each buffer stores its vertices
//! relative to an [`anchor`](MeshBuffer::anchor), the centre of the
//! geometry's own bounding box, so `f32` keeps sub-millimetre precision even
//! for georeferenced vertices (upstream samples reach 4.9 × 10⁶ m in mesh
//! points). The scene's [`origin`](RenderScene::origin) is subtracted from
//! world coordinates; by default it is the centre of the world bounds. Each
//! instance's render [`matrix`](Instance::matrix) maps buffer coordinates to
//! world coordinates minus the origin: anchor, then world transform, then
//! origin shift, multiplied in `f64` and rounded to `f32` last. Add the
//! origin back, for example as a root translation, to place the scene in the
//! file's coordinates.
//!
//! # Differences from the reference viewer
//!
//! The reference viewer is `src/viewer/render.ts` of
//! buildingSMART/IFC5-development at `1a63082`.
//!
//! - A node with a mesh, curves, and a point cloud yields all three; the
//!   viewer draws only the first of mesh, curves, point cloud. No upstream
//!   example has two kinds on one node.
//! - The viewer applies no transform to the root it is given, which is always
//!   the artificial root without attributes. [`RenderScene::from_root`]
//!   applies the given node's transform like any other.
//! - The viewer draws lines in 0.8 times the resolved colour. The scene keeps
//!   the resolved colour; the darkening is a styling choice for renderers.
//! - The viewer draws `pcd::base64` points without colours in white, through
//!   three.js `PCDLoader`'s default, and other points without colours in
//!   black. The scene uses black for all of them.
//! - The viewer computes vertex normals with three.js
//!   `computeVertexNormals`; so does the scene, area-weighted over the
//!   triangles that share a vertex. A vertex in no triangle with an area
//!   gets `+Z`, so that every normal is a unit vector.
//! - See [`presentation`](crate::presentation) for opacity `0`,
//!   `visibility: "inherited"`, and all-`false` glTF materials.
//!
//! # Mirroring
//!
//! A world transform with a negative determinant mirrors its geometry and
//! reverses the winding of its triangles. Buffers are shared between
//! instances and keep the file's winding; [`Instance::mirrored`] reports
//! these instances, and renderers flip their front face (glTF viewers and
//! three.js do so from the node matrix).
//!
//! # Warnings
//!
//! Building a scene does not fail. A value that cannot be decoded becomes a
//! [`SceneWarning`] with the node path and attribute id, and only that value
//! is skipped: a malformed mesh leaves out the mesh, a malformed colour falls
//! back to the inherited one. A malformed `usd::xformop` leaves out the node
//! with its subtree, since nothing below it can be placed. Linear curves of
//! an unsupported `type` are skipped with a warning as well. Each distinct
//! value is reported once, at the first path that reaches it. Empty
//! geometry is skipped without a warning.

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use openbim_ifcx::{ComposedNode, Composition};
use serde_json::Value;

use crate::curves::{self, CurveGeometry, UnsupportedCurve};
use crate::math::{cross, sub, Vec3};
use crate::mesh::{self, TriangleMesh};
use crate::points::{PointCloud, PCD_BASE64, POINTS_ARRAY, POINTS_BASE64};
use crate::presentation::{BasicMaterial, Material, NodePresentation, Visibility};
use crate::transform::{self, world_from_parent, Transform};
use crate::DecodeError;

/// Where the render origin of a scene lies.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Origin {
    /// The centre of the scene's world bounds, or zero for an empty scene.
    #[default]
    Auto,
    /// A fixed point in world coordinates; `At([0.0; 3])` disables the
    /// shift.
    At(Vec3),
}

/// Options for [`RenderScene::from_composition`].
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct SceneOptions {
    /// The render origin; [`Origin::Auto`] by default.
    pub origin: Origin,
}

impl SceneOptions {
    /// Sets the render origin.
    pub fn with_origin(mut self, origin: Origin) -> Self {
        self.origin = origin;
        self
    }
}

/// An axis-aligned box in `f64`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min: Vec3,
    pub max: Vec3,
}

impl Bounds {
    /// The smallest box containing every point, or `None` for none.
    pub fn from_points<I: IntoIterator<Item = Vec3>>(points: I) -> Option<Self> {
        let mut points = points.into_iter();
        let first = points.next()?;
        let mut bounds = Self {
            min: first,
            max: first,
        };
        for p in points {
            bounds.include(p);
        }
        Some(bounds)
    }

    /// Grows the box to contain `p`.
    pub fn include(&mut self, p: Vec3) {
        for (i, c) in p.into_iter().enumerate() {
            self.min[i] = self.min[i].min(c);
            self.max[i] = self.max[i].max(c);
        }
    }

    /// The smallest box containing both.
    pub fn union(&self, other: &Bounds) -> Bounds {
        let mut out = *self;
        out.include(other.min);
        out.include(other.max);
        out
    }

    /// The centre.
    pub fn center(&self) -> Vec3 {
        [0, 1, 2].map(|i| (self.min[i] + self.max[i]) / 2.0)
    }

    /// The edge lengths.
    pub fn size(&self) -> Vec3 {
        [0, 1, 2].map(|i| self.max[i] - self.min[i])
    }
}

/// A shared triangle mesh, from `usd::usdgeom::mesh`.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshBuffer {
    /// Local coordinates of the buffer's zero: the centre of the mesh's
    /// local bounding box. A vertex's local position is `anchor + position`.
    pub anchor: Vec3,
    /// Vertex positions relative to `anchor`.
    pub positions: Vec<[f32; 3]>,
    /// One unit normal per vertex, area-weighted over its triangles.
    pub normals: Vec<[f32; 3]>,
    /// Three indices per triangle, counter-clockwise front faces.
    pub indices: Vec<u32>,
}

impl MeshBuffer {
    /// Number of triangles.
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

/// Shared line segments, from the linear curves of
/// `usd::usdgeom::basiscurves`.
#[derive(Debug, Clone, PartialEq)]
pub struct LineBuffer {
    /// See [`MeshBuffer::anchor`].
    pub anchor: Vec3,
    /// Vertex positions relative to `anchor`, every polyline in order.
    pub positions: Vec<[f32; 3]>,
    /// Two indices per segment. A closed polyline adds a segment from its
    /// last vertex back to its first.
    pub indices: Vec<u32>,
}

impl LineBuffer {
    /// Number of line segments.
    pub fn segment_count(&self) -> usize {
        self.indices.len() / 2
    }
}

/// Shared points, from `pcd::base64`, `points::array`, or `points::base64`.
#[derive(Debug, Clone, PartialEq)]
pub struct PointBuffer {
    /// See [`MeshBuffer::anchor`].
    pub anchor: Vec3,
    /// Point positions relative to `anchor`.
    pub positions: Vec<[f32; 3]>,
    /// One linear RGB colour per point, if the source has colours.
    pub colors: Option<Vec<[f32; 3]>>,
}

/// What an instance draws: an index into one of the scene's buffer lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GeometryRef {
    /// Index into [`RenderScene::meshes`].
    Mesh(usize),
    /// Index into [`RenderScene::lines`].
    Lines(usize),
    /// Index into [`RenderScene::points`].
    Points(usize),
}

/// The kind of geometry an instance draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GeometryKind {
    Mesh,
    Lines,
    Points,
}

impl GeometryRef {
    /// The kind of buffer referenced.
    pub fn kind(&self) -> GeometryKind {
        match self {
            Self::Mesh(_) => GeometryKind::Mesh,
            Self::Lines(_) => GeometryKind::Lines,
            Self::Points(_) => GeometryKind::Points,
        }
    }
}

/// One drawn geometry of one node at one path.
#[derive(Debug, Clone, PartialEq)]
pub struct Instance {
    /// Child names from the root, joined by `/`, for example
    /// `site/Storey/Wall`. Resolve it with [`Composition::get`] to show the
    /// node's attributes on pick. Unique per node, shared by the instances
    /// of one node with several kinds of geometry.
    pub path: String,
    /// The path of the node that defined this node, [`ComposedNode::path`].
    pub node: String,
    /// The shared geometry drawn.
    pub geometry: GeometryRef,
    /// Index into [`RenderScene::materials`].
    pub material: usize,
    /// The node's world transform in the file's coordinates.
    pub world: Transform,
    /// Column-major `f32` matrix from buffer coordinates to world
    /// coordinates minus the scene origin, the layout of glTF `node.matrix`.
    pub matrix: [f32; 16],
    /// World bounds of the drawn vertices, in the file's coordinates.
    pub bounds: Bounds,
}

impl Instance {
    /// Whether the world transform mirrors, so triangle winding is reversed.
    pub fn mirrored(&self) -> bool {
        self.world.determinant() < 0.0
    }
}

/// Why part of a node was left out of a scene.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum SceneWarningKind {
    /// The attribute value could not be decoded.
    Decode(DecodeError),
    /// Curves of a type that is not drawn yet.
    UnsupportedCurve(UnsupportedCurve),
}

/// A value skipped while building a scene; see the [module docs](self).
#[derive(Debug, Clone, PartialEq)]
pub struct SceneWarning {
    /// The first instance path at which the value was found.
    pub path: String,
    /// The attribute id, for example `usd::usdgeom::mesh`.
    pub attribute: &'static str,
    pub kind: SceneWarningKind,
}

impl fmt::Display for SceneWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}: ", display_path(&self.path), self.attribute)?;
        match &self.kind {
            SceneWarningKind::Decode(e) => write!(f, "{e}"),
            SceneWarningKind::UnsupportedCurve(c) => {
                write!(f, "curve type {:?} is not supported", c.curve_type)?;
                if let Some(basis) = &c.basis {
                    write!(f, " (basis {basis:?})")?;
                }
                Ok(())
            }
        }
    }
}

fn display_path(path: &str) -> &str {
    if path.is_empty() {
        "<root>"
    } else {
        path
    }
}

/// A flat render scene; see the [module docs](self).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RenderScene {
    /// Drawn geometry, parents before children.
    pub instances: Vec<Instance>,
    pub meshes: Vec<MeshBuffer>,
    pub lines: Vec<LineBuffer>,
    pub points: Vec<PointBuffer>,
    pub materials: Vec<Material>,
    /// World bounds of every instance, `None` for an empty scene.
    pub bounds: Option<Bounds>,
    /// World coordinates of the render origin; instance matrices map to
    /// world coordinates minus this point.
    pub origin: Vec3,
    pub warnings: Vec<SceneWarning>,
}

impl RenderScene {
    /// Builds the scene of a composition under its artificial root
    /// ([`Composition::root`]).
    ///
    /// ```
    /// use openbim_ifcx::{compose, flatten, IfcxNode};
    /// use openbim_ifcx_geometry::scene::{GeometryRef, RenderScene, SceneOptions};
    ///
    /// let nodes: Vec<IfcxNode> = serde_json::from_str(r#"[
    ///     {"path": "panel", "attributes": {"usd::usdgeom::mesh": {
    ///         "points": [[0, 0, 0], [1, 0, 0], [0, 1, 0]], "faceVertexIndices": [0, 1, 2]}}},
    ///     {"path": "a", "inherits": {"type": "panel"}},
    ///     {"path": "b", "inherits": {"type": "panel"}, "attributes": {"usd::xformop":
    ///         {"transform": [[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [5, 0, 0, 1]]}}}
    /// ]"#)?;
    /// let composition = compose(&flatten(&nodes))?;
    /// let scene = RenderScene::from_composition(&composition, &SceneOptions::default());
    /// assert_eq!(scene.meshes.len(), 1, "stored once");
    /// let paths: Vec<_> = scene.instances.iter().map(|i| i.path.as_str()).collect();
    /// assert_eq!(paths, ["a", "b"]);
    /// assert_eq!(scene.instances[1].geometry, GeometryRef::Mesh(0));
    /// let bounds = scene.bounds.unwrap();
    /// assert_eq!((bounds.min, bounds.max), ([0.0; 3], [6.0, 1.0, 0.0]));
    /// assert_eq!(scene.origin, [3.0, 0.5, 0.0]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_composition(composition: &Composition, options: &SceneOptions) -> Self {
        Self::from_root(&composition.root(), options)
    }

    /// Builds the scene of `root` and everything below it. Instance paths
    /// are relative to `root`, which itself has path `""`; its own
    /// attributes, including a transform, apply like those of any node.
    pub fn from_root(root: &ComposedNode, options: &SceneOptions) -> Self {
        let mut builder = Builder::default();
        builder.walk(root);
        builder.finish(options)
    }

    /// Number of triangles drawn, counting every instance.
    pub fn triangle_count(&self) -> usize {
        self.instances
            .iter()
            .map(|i| match i.geometry {
                GeometryRef::Mesh(m) => self.meshes[m].triangle_count(),
                _ => 0,
            })
            .sum()
    }

    /// Number of line segments drawn, counting every instance.
    pub fn segment_count(&self) -> usize {
        self.instances
            .iter()
            .map(|i| match i.geometry {
                GeometryRef::Lines(l) => self.lines[l].segment_count(),
                _ => 0,
            })
            .sum()
    }

    /// Number of points drawn, counting every instance.
    pub fn point_count(&self) -> usize {
        self.instances
            .iter()
            .map(|i| match i.geometry {
                GeometryRef::Points(p) => self.points[p].positions.len(),
                _ => 0,
            })
            .sum()
    }

    /// The instances drawn for the node at `path`, at most one per
    /// geometry kind. They are adjacent because instances are listed in
    /// walk order. Linear in the number of instances.
    pub fn instances_at(&self, path: &str) -> &[Instance] {
        let Some(start) = self.instances.iter().position(|i| i.path == path) else {
            return &[];
        };
        let len = self.instances[start..]
            .iter()
            .take_while(|i| i.path == path)
            .count();
        &self.instances[start..start + len]
    }

    fn anchor(&self, geometry: GeometryRef) -> Vec3 {
        match geometry {
            GeometryRef::Mesh(i) => self.meshes[i].anchor,
            GeometryRef::Lines(i) => self.lines[i].anchor,
            GeometryRef::Points(i) => self.points[i].anchor,
        }
    }
}

/// A node's own decoded attributes, cached per composed node.
#[derive(Clone, Copy)]
struct NodeInfo {
    /// `None`: leave the node and its subtree out.
    local: Option<Option<Transform>>,
    invisible: bool,
    pbr: Option<usize>,
    basic: Option<usize>,
    mesh: Option<usize>,
    lines: Option<usize>,
    points: Option<(usize, bool)>,
}

/// What a node passes on to its children.
#[derive(Clone, Copy)]
struct Context {
    world: Transform,
    pbr: Option<usize>,
    basic: Option<usize>,
}

/// Raw pointers serve only as identities here; they are never dereferenced.
type Key = *const ();

#[derive(Default)]
struct Builder {
    scene: RenderScene,
    nodes: HashMap<Key, NodeInfo>,
    meshes: HashMap<Key, Option<usize>>,
    lines: HashMap<Key, Option<usize>>,
    points: HashMap<Key, Option<(usize, bool)>>,
    materials: HashMap<String, usize>,
    /// Local `f64` vertices of each buffer, for exact world bounds.
    local: HashMap<GeometryRef, Vec<Vec3>>,
}

fn key<T>(value: &T) -> Key {
    (value as *const T).cast()
}

impl Builder {
    fn walk(&mut self, root: &ComposedNode) {
        let mut stack: Vec<(&ComposedNode, String, Context)> = vec![(
            root,
            String::new(),
            Context {
                world: Transform::IDENTITY,
                pbr: None,
                basic: None,
            },
        )];
        while let Some((node, path, parent)) = stack.pop() {
            let info = self.info(node, &path);
            let Some(local) = info.local else { continue };
            if info.invisible {
                continue;
            }
            let context = Context {
                world: world_from_parent(&parent.world, local.as_ref()),
                pbr: info.pbr.or(parent.pbr),
                basic: info.basic.or(parent.basic),
            };
            let basic = || context.basic;
            if let Some(mesh) = info.mesh {
                let material = match context.pbr.or(basic()) {
                    Some(m) => m,
                    None => self.material(Material::Basic(BasicMaterial::DEFAULT)),
                };
                self.instance(&path, node, GeometryRef::Mesh(mesh), material, &context);
            }
            if let Some(lines) = info.lines {
                let material = match basic() {
                    Some(m) => m,
                    None => self.material(Material::Basic(BasicMaterial::DEFAULT)),
                };
                self.instance(&path, node, GeometryRef::Lines(lines), material, &context);
            }
            if let Some((points, colored)) = info.points {
                let material = self.material(Material::Basic(if colored {
                    POINT_COLORS
                } else {
                    POINT_DEFAULT
                }));
                self.instance(&path, node, GeometryRef::Points(points), material, &context);
            }
            for (name, child) in node.children.iter().rev() {
                let child_path = if path.is_empty() {
                    name.clone()
                } else {
                    format!("{path}/{name}")
                };
                stack.push((child, child_path, context));
            }
        }
    }

    fn instance(
        &mut self,
        path: &str,
        node: &ComposedNode,
        geometry: GeometryRef,
        material: usize,
        context: &Context,
    ) {
        self.scene.instances.push(Instance {
            path: path.to_owned(),
            node: node.path.clone(),
            geometry,
            material,
            world: context.world,
            matrix: [0.0; 16],
            bounds: Bounds {
                min: [0.0; 3],
                max: [0.0; 3],
            },
        });
    }

    fn material(&mut self, material: Material) -> usize {
        let materials = &mut self.scene.materials;
        *self
            .materials
            .entry(format!("{material:?}"))
            .or_insert_with(|| {
                materials.push(material);
                materials.len() - 1
            })
    }

    fn warn(&mut self, path: &str, attribute: &'static str, kind: SceneWarningKind) {
        self.scene.warnings.push(SceneWarning {
            path: path.to_owned(),
            attribute,
            kind,
        });
    }

    fn info(&mut self, node: &ComposedNode, path: &str) -> NodeInfo {
        if let Some(info) = self.nodes.get(&key(node)) {
            return *info;
        }
        let attribute = |id: &str| node.attributes.get(id).filter(|v| !v.is_null());

        let local = match attribute(transform::ATTRIBUTE) {
            None => Some(None),
            Some(value) => match Transform::from_attribute(value) {
                Ok(t) => Some(Some(t)),
                Err(e) => {
                    self.warn(path, transform::ATTRIBUTE, SceneWarningKind::Decode(e));
                    None
                }
            },
        };

        let mut errors = Vec::new();
        let presentation = NodePresentation::from_attributes_lenient(node, &mut errors);
        for (attribute, e) in errors {
            self.warn(path, attribute, SceneWarningKind::Decode(e));
        }
        let pbr = presentation
            .gltf_material
            .map(|m| self.material(Material::Pbr(m)));
        let basic = presentation.diffuse_color.map(|color| {
            self.material(Material::Basic(BasicMaterial {
                color,
                opacity: presentation.opacity.unwrap_or(1.0),
            }))
        });

        let mesh = attribute(mesh::ATTRIBUTE).and_then(|v| self.mesh(v, path));
        let lines = attribute(curves::ATTRIBUTE).and_then(|v| self.lines(v, path));
        let points = [PCD_BASE64, POINTS_ARRAY, POINTS_BASE64]
            .into_iter()
            .find_map(|id| attribute(id).map(|v| (id, v)))
            .and_then(|(id, v)| self.points(id, v, path));

        let info = NodeInfo {
            local,
            invisible: presentation.visibility == Some(Visibility::Invisible),
            pbr,
            basic,
            mesh,
            lines,
            points,
        };
        self.nodes.insert(key(node), info);
        info
    }

    fn mesh(&mut self, value: &Arc<Value>, path: &str) -> Option<usize> {
        if let Some(id) = self.meshes.get(&key(&**value)) {
            return *id;
        }
        let id = match TriangleMesh::from_attribute(value) {
            Ok(mesh) => mesh_buffer(&mesh).map(|buffer| {
                self.scene.meshes.push(buffer);
                let id = self.scene.meshes.len() - 1;
                self.local.insert(GeometryRef::Mesh(id), mesh.positions);
                id
            }),
            Err(e) => {
                self.warn(path, mesh::ATTRIBUTE, SceneWarningKind::Decode(e));
                None
            }
        };
        self.meshes.insert(key(&**value), id);
        id
    }

    fn lines(&mut self, value: &Arc<Value>, path: &str) -> Option<usize> {
        if let Some(id) = self.lines.get(&key(&**value)) {
            return *id;
        }
        let id = match CurveGeometry::from_attribute(value) {
            Ok(CurveGeometry::Polylines(polylines)) => line_buffer(&polylines).map(|buffer| {
                self.scene.lines.push(buffer);
                let id = self.scene.lines.len() - 1;
                let local = polylines.into_iter().flat_map(|l| l.points).collect();
                self.local.insert(GeometryRef::Lines(id), local);
                id
            }),
            Ok(CurveGeometry::Unsupported(curve)) => {
                self.warn(
                    path,
                    curves::ATTRIBUTE,
                    SceneWarningKind::UnsupportedCurve(curve),
                );
                None
            }
            Err(e) => {
                self.warn(path, curves::ATTRIBUTE, SceneWarningKind::Decode(e));
                None
            }
        };
        self.lines.insert(key(&**value), id);
        id
    }

    fn points(
        &mut self,
        id: &'static str,
        value: &Arc<Value>,
        path: &str,
    ) -> Option<(usize, bool)> {
        if let Some(entry) = self.points.get(&key(&**value)) {
            return *entry;
        }
        let decoded = match id {
            PCD_BASE64 => PointCloud::from_pcd_base64(value),
            POINTS_ARRAY => PointCloud::from_points_array(value),
            _ => PointCloud::from_points_base64(value),
        };
        let entry = match decoded {
            Ok(mut cloud) => {
                let buffer = point_buffer(&mut cloud);
                let positions = cloud.positions;
                buffer.map(|buffer| {
                    let colored = buffer.colors.is_some();
                    self.scene.points.push(buffer);
                    let id = self.scene.points.len() - 1;
                    self.local.insert(GeometryRef::Points(id), positions);
                    (id, colored)
                })
            }
            Err(e) => {
                self.warn(path, id, SceneWarningKind::Decode(e));
                None
            }
        };
        self.points.insert(key(&**value), entry);
        entry
    }

    fn finish(mut self, options: &SceneOptions) -> RenderScene {
        let mut scene = std::mem::take(&mut self.scene);
        let mut world_bounds: Option<Bounds> = None;
        let mut instance_bounds = Vec::with_capacity(scene.instances.len());
        for instance in &scene.instances {
            let positions = &self.local[&instance.geometry];
            let bounds =
                Bounds::from_points(positions.iter().map(|&p| instance.world.transform_point(p)))
                    .expect("empty buffers are never instanced");
            world_bounds = Some(world_bounds.map_or(bounds, |b| b.union(&bounds)));
            instance_bounds.push(bounds);
        }
        scene.bounds = world_bounds;
        scene.origin = match options.origin {
            Origin::Auto => world_bounds.map_or([0.0; 3], |b| b.center()),
            Origin::At(origin) => origin,
        };
        let shift = Transform::from_translation(scene.origin.map(|c| -c));
        let anchors: Vec<Vec3> = scene
            .instances
            .iter()
            .map(|i| scene.anchor(i.geometry))
            .collect();
        for ((instance, bounds), anchor) in
            scene.instances.iter_mut().zip(instance_bounds).zip(anchors)
        {
            let render = Transform::from_translation(anchor)
                .then(&instance.world)
                .then(&shift);
            instance.matrix = render.to_column_major().map(|v| v as f32);
            instance.bounds = bounds;
        }
        scene
    }
}

/// Points with colours: white, so the per-point colours show unchanged.
const POINT_COLORS: BasicMaterial = BasicMaterial {
    color: [1.0; 3],
    opacity: 1.0,
};

/// Points without colours: black, as in the reference viewer.
const POINT_DEFAULT: BasicMaterial = BasicMaterial {
    color: [0.0; 3],
    opacity: 1.0,
};

/// The centre of the bounding box, and `points` relative to it in `f32`.
fn anchored(points: impl Iterator<Item = Vec3> + Clone) -> Option<(Vec3, Vec<[f32; 3]>)> {
    let anchor = Bounds::from_points(points.clone())?.center();
    let relative = points
        .map(|p| [0, 1, 2].map(|k| (p[k] - anchor[k]) as f32))
        .collect();
    Some((anchor, relative))
}

fn mesh_buffer(mesh: &TriangleMesh) -> Option<MeshBuffer> {
    if mesh.indices.is_empty() {
        return None;
    }
    let (anchor, positions) = anchored(mesh.positions.iter().copied())?;
    // three.js computeVertexNormals: sum the unnormalised face normals,
    // which weights each by twice the triangle's area.
    let mut sums = vec![[0.0f64; 3]; mesh.positions.len()];
    for t in mesh.triangles() {
        let [a, b, c] = t.map(|i| mesh.positions[i as usize]);
        let n = cross(sub(b, a), sub(c, a));
        for i in t {
            let sum = &mut sums[i as usize];
            for k in 0..3 {
                sum[k] += n[k];
            }
        }
    }
    let normals = sums
        .into_iter()
        .map(|n| {
            let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
            if len > 0.0 && len.is_finite() {
                n.map(|c| (c / len) as f32)
            } else {
                [0.0, 0.0, 1.0]
            }
        })
        .collect();
    Some(MeshBuffer {
        anchor,
        positions,
        normals,
        indices: mesh.indices.clone(),
    })
}

fn line_buffer(polylines: &[curves::Polyline]) -> Option<LineBuffer> {
    let (anchor, positions) = anchored(polylines.iter().flat_map(|l| l.points.iter().copied()))?;
    let mut indices = Vec::new();
    let mut start = 0u32;
    for line in polylines {
        let n = line.points.len() as u32;
        for i in 0..n - 1 {
            indices.extend([start + i, start + i + 1]);
        }
        if line.closed && n > 2 {
            indices.extend([start + n - 1, start]);
        }
        start += n;
    }
    Some(LineBuffer {
        anchor,
        positions,
        indices,
    })
}

fn point_buffer(cloud: &mut PointCloud) -> Option<PointBuffer> {
    let (anchor, positions) = anchored(cloud.positions.iter().copied())?;
    Some(PointBuffer {
        anchor,
        positions,
        colors: cloud.colors.take(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertex_normals_are_area_weighted_and_unit() {
        // Two triangles share the edge 0-1: a large one in the XY plane and
        // a small one in the XZ plane. Vertex 3 is in no triangle.
        let mesh = TriangleMesh::new(
            vec![
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [0.0, 3.0, 0.0],
                [9.0, 9.0, 9.0],
                [0.0, 0.0, -1.0],
            ],
            vec![0, 1, 2, 0, 4, 1],
        )
        .unwrap();
        let buffer = mesh_buffer(&mesh).unwrap();
        // Face normals (unnormalised): (0, 0, 3) and (0, -1, 0).
        let shared = [0.0, -1.0 / 10f64.sqrt(), 3.0 / 10f64.sqrt()].map(|c| c as f32);
        assert_eq!(buffer.normals[0], shared);
        assert_eq!(buffer.normals[1], shared);
        assert_eq!(buffer.normals[2], [0.0, 0.0, 1.0]);
        assert_eq!(buffer.normals[3], [0.0, 0.0, 1.0], "unused vertex");
        assert_eq!(buffer.normals[4], [0.0, -1.0, 0.0]);
        assert_eq!(buffer.anchor, [4.5, 4.5, 4.0]);
    }

    #[test]
    fn empty_geometry_makes_no_buffer() {
        let mesh = TriangleMesh::new(vec![[0.0; 3]], vec![]).unwrap();
        assert!(mesh_buffer(&mesh).is_none());
        assert!(line_buffer(&[]).is_none());
    }
}
