//! Composition: expand flattened nodes into a resolved tree.
//!
//! This is the second half of composition, upstream's `ComposeNode`,
//! `FindRootsOrCycles`, and `CreateArtificialRoot`
//! (`src/ifcx-core/composition/{compose,cycles}.ts` in
//! buildingSMART/IFC5-development).

use std::error::Error;
use std::fmt;
use std::sync::Arc;

use indexmap::IndexMap;
use serde_json::Value;

use super::flatten::FlatNode;

/// One node of the composed tree. Upstream calls this a post-composition
/// node.
///
/// Children are shared through [`Arc`]: every instance that inherits a type
/// points at the same composed sub-tree instead of a copy. A sub-tree is
/// copied, one node at a time along the edited path, only where a path such
/// as `instance/Body` edits it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ComposedNode {
    /// The path whose opinions started this node, upstream's `node` field.
    /// A child reached through `children: {"Body": "b1"}` has path `b1`; a
    /// child copied from an inherited node keeps that node's path. Empty for
    /// the artificial root.
    pub path: String,
    /// Inherited attributes first, then local ones, which win.
    pub attributes: IndexMap<String, Arc<Value>>,
    pub children: IndexMap<String, Arc<ComposedNode>>,
}

impl ComposedNode {
    /// Follows child names separated by `/`, as upstream's
    /// `GetChildNodeWithPath`. An empty path returns this node.
    pub fn descendant(self: &Arc<Self>, path: &str) -> Option<&Arc<ComposedNode>> {
        let mut node = self;
        let mut rest = path;
        loop {
            if rest.is_empty() {
                return Some(node);
            }
            let (name, tail) = match rest.split_once('/') {
                Some((name, tail)) => (name, Some(tail)),
                None => (rest, None),
            };
            node = node.children.get(name)?;
            match tail {
                Some(tail) => rest = tail,
                None => return Some(node),
            }
        }
    }
}

impl Drop for ComposedNode {
    // The default drop recurses once per tree level. Unwind children that
    // nothing else shares with an explicit stack instead.
    fn drop(&mut self) {
        let mut stack: Vec<Arc<ComposedNode>> =
            self.children.drain(..).map(|(_, child)| child).collect();
        while let Some(child) = stack.pop() {
            if let Some(mut child) = Arc::into_inner(child) {
                stack.extend(child.children.drain(..).map(|(_, c)| c));
            }
        }
    }
}

/// Why flattened nodes could not be composed.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ComposeError {
    /// References loop back to where they started. `cycle` lists head paths
    /// (the part before the first `/`); each one references the next and
    /// the last references the first. It is never empty.
    Cycle { cycle: Vec<String> },
    /// `reference`, named in `inherits` or `children` of the node at `path`,
    /// names no node, or names a child that does not exist.
    UnknownReference { path: String, reference: String },
}

impl fmt::Display for ComposeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cycle { cycle } => {
                write!(f, "reference cycle: ")?;
                for path in cycle {
                    write!(f, "{path} -> ")?;
                }
                write!(f, "{}", cycle.first().map_or("", String::as_str))
            }
            Self::UnknownReference { path, reference } => {
                write!(f, "node {path} references unknown node {reference}")
            }
        }
    }
}

impl Error for ComposeError {}

/// The composed tree of a set of flattened nodes.
#[derive(Debug, Clone)]
pub struct Composition {
    /// Composed node per head path, in order of first appearance.
    heads: IndexMap<String, Arc<ComposedNode>>,
    roots: Vec<String>,
}

impl Composition {
    /// Paths without `/` that no `inherits` or `children` reference names,
    /// in order of first appearance.
    pub fn roots(&self) -> &[String] {
        &self.roots
    }

    /// The composed node at `path`, which is a node path optionally followed
    /// by child names: `wall`, `wall/Body`, `wall/Body/Mesh`.
    pub fn get(&self, path: &str) -> Option<&Arc<ComposedNode>> {
        let (head, tail) = split_head(path);
        self.heads.get(head)?.descendant(tail)
    }

    /// An artificial root with path `""`, no attributes, and every root as
    /// a child named by its path, as upstream's `CreateArtificialRoot`.
    pub fn root(&self) -> ComposedNode {
        ComposedNode {
            path: String::new(),
            attributes: IndexMap::new(),
            children: self
                .roots
                .iter()
                .map(|root| (root.clone(), self.heads[root.as_str()].clone()))
                .collect(),
        }
    }
}

/// `head/a/b` to (`head`, `a/b`), upstream's `GetHead` and `GetTail`.
fn split_head(path: &str) -> (&str, &str) {
    path.split_once('/').unwrap_or((path, ""))
}

/// Every flattened node whose path starts with one head.
struct Head<'a> {
    /// The node at the head path itself, if any.
    node: Option<&'a FlatNode>,
    /// Nodes at `head/...`, which edit children of the composed head.
    edits: Vec<(&'a str, &'a FlatNode)>,
    /// Heads this head's references name, by index; unknown ones left out.
    deps: Vec<usize>,
}

impl<'a> Head<'a> {
    fn nodes(&self) -> impl Iterator<Item = &'a FlatNode> + '_ {
        self.node.into_iter().chain(self.edits.iter().map(|e| e.1))
    }
}

fn references(node: &FlatNode) -> impl Iterator<Item = &str> {
    node.inherits
        .values()
        .chain(node.children.values().flatten())
        .map(String::as_str)
}

/// Composes flattened nodes into a tree, as upstream's `LoadIfcxFile` does
/// after flattening.
///
/// For each node, in this order:
///
/// 1. each `inherits` entry, in order, copies the children and attributes of
///    the node it references;
/// 2. each `children` entry adds or replaces a child with the node it
///    references, and a `None` entry deletes the child, inherited or not;
/// 3. local attributes override inherited ones.
///
/// A reference is a node path optionally followed by child names, such as
/// `type/Body`; the child names are looked up in the composed node. Then
/// nodes with paths such as `wall/Body` are applied to the matching child,
/// parents before children; one whose child does not exist is ignored.
///
/// Every head path is composed once and shared. A node costs the size of its
/// own maps plus the top-level maps of the nodes it inherits, not the size of
/// any sub-tree, and an edit such as `wall/Body` copies only the nodes along
/// its path. Nothing recurses per tree level, including dropping the result.
///
/// # Errors
///
/// [`ComposeError::Cycle`] if references form a cycle, checked before
/// anything is composed, and [`ComposeError::UnknownReference`] for a
/// reference that does not resolve.
///
/// # Differences from upstream
///
/// - Cycles and roots look at the head of each reference, the part before
///   the first `/`, as upstream's own `TODO` in `FindRootsOrCycles` asks.
///   Upstream compares whole reference strings, so it misses cycles through
///   `head/child` references and recurses until the stack overflows, and it
///   lists a node referenced only as `head/child` as a root as well.
/// - A reference to a path with no node is an error. Upstream composes it as
///   an empty node.
///
/// ```
/// use openbim_ifcx::{compose, flatten, IfcxNode};
///
/// let nodes: Vec<IfcxNode> = serde_json::from_str(r#"[
///     {"path": "type", "children": {"Body": "mesh"}, "attributes": {"x::kind": "T"}},
///     {"path": "mesh", "attributes": {"x::points": [0, 1, 2]}},
///     {"path": "wall", "inherits": {"Type": "type"}, "attributes": {"x::kind": "W"}}
/// ]"#)?;
/// let composed = compose(&flatten(&nodes))?;
/// assert_eq!(composed.roots(), ["wall"]);
/// let wall = composed.get("wall").unwrap();
/// assert_eq!(*wall.attributes["x::kind"], "W");
/// // The body is the composed `mesh` node itself, not a copy.
/// assert!(std::sync::Arc::ptr_eq(&wall.children["Body"], composed.get("mesh").unwrap()));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn compose(flat: &IndexMap<String, FlatNode>) -> Result<Composition, ComposeError> {
    let mut heads: IndexMap<&str, Head<'_>> = IndexMap::new();
    for (path, node) in flat {
        let (head, _) = split_head(path);
        let entry = heads.entry(head).or_insert_with(|| Head {
            node: None,
            edits: Vec::new(),
            deps: Vec::new(),
        });
        if head.len() == path.len() {
            entry.node = Some(node);
        } else {
            entry.edits.push((path.as_str(), node));
        }
    }

    let mut referenced = vec![false; heads.len()];
    for index in 0..heads.len() {
        let mut deps: Vec<usize> = heads[index]
            .nodes()
            .flat_map(references)
            .filter_map(|reference| heads.get_index_of(split_head(reference).0))
            .collect();
        deps.sort_unstable();
        deps.dedup();
        for &dep in &deps {
            referenced[dep] = true;
        }
        heads[index].deps = deps;
    }

    let order = topological_order(&heads)?;

    let mut composed: Vec<Option<Arc<ComposedNode>>> = vec![None; heads.len()];
    for index in order {
        let (path, head) = heads.get_index(index).expect("index from heads");
        let node = compose_head(path, head, &heads, &composed)?;
        composed[index] = Some(Arc::new(node));
    }

    let roots = heads
        .iter()
        .enumerate()
        .filter(|(index, (_, head))| head.node.is_some() && !referenced[*index])
        .map(|(_, (path, _))| (*path).to_owned())
        .collect();
    let heads = heads
        .keys()
        .zip(composed)
        .map(|(path, node)| ((*path).to_owned(), node.expect("every head composed")))
        .collect();
    Ok(Composition { heads, roots })
}

/// Head indices with every head after the heads it references, or the
/// first cycle found. Iterative depth-first search.
fn topological_order(heads: &IndexMap<&str, Head<'_>>) -> Result<Vec<usize>, ComposeError> {
    const NEW: u8 = 0;
    const ACTIVE: u8 = 1;
    const DONE: u8 = 2;
    let mut state = vec![NEW; heads.len()];
    let mut order = Vec::with_capacity(heads.len());
    // (head, index of the next dependency to visit)
    let mut stack: Vec<(usize, usize)> = Vec::new();
    for start in 0..heads.len() {
        if state[start] != NEW {
            continue;
        }
        state[start] = ACTIVE;
        stack.push((start, 0));
        while let Some((index, next)) = stack.last_mut() {
            let deps = &heads[*index].deps;
            if let Some(&dep) = deps.get(*next) {
                *next += 1;
                match state[dep] {
                    NEW => {
                        state[dep] = ACTIVE;
                        stack.push((dep, 0));
                    }
                    ACTIVE => {
                        let from = stack.iter().position(|(i, _)| *i == dep).expect("active");
                        let cycle = stack[from..]
                            .iter()
                            .map(|(i, _)| (*heads.get_index(*i).expect("index").0).to_owned())
                            .collect();
                        return Err(ComposeError::Cycle { cycle });
                    }
                    _ => {}
                }
            } else {
                state[*index] = DONE;
                order.push(*index);
                stack.pop();
            }
        }
    }
    Ok(order)
}

fn compose_head(
    path: &str,
    head: &Head<'_>,
    heads: &IndexMap<&str, Head<'_>>,
    composed: &[Option<Arc<ComposedNode>>],
) -> Result<ComposedNode, ComposeError> {
    let mut node = ComposedNode {
        path: path.to_owned(),
        attributes: IndexMap::new(),
        children: IndexMap::new(),
    };
    if let Some(flat) = head.node {
        add_data(&mut node, path, flat, heads, composed)?;
    }

    // Upstream walks the composed tree and applies the node at each
    // `head/a/b` it passes. Applying edits by depth, parents first, gives the
    // same result: an edit only changes its own sub-tree.
    let mut edits: Vec<&(&str, &FlatNode)> = head.edits.iter().collect();
    edits.sort_by_key(|(edit_path, _)| edit_path.matches('/').count());
    for (edit_path, flat) in edits {
        let names = split_head(edit_path).1;
        let mut target = &node;
        let mut found = true;
        for name in names.split('/') {
            match target.children.get(name) {
                Some(child) => target = child,
                None => {
                    found = false;
                    break;
                }
            }
        }
        if !found {
            continue;
        }
        let mut target = &mut node;
        for name in names.split('/') {
            let child = target.children.get_mut(name).expect("checked above");
            target = Arc::make_mut(child);
        }
        add_data(target, edit_path, flat, heads, composed)?;
    }
    Ok(node)
}

/// Upstream's `AddDataFromPreComposition`.
fn add_data(
    node: &mut ComposedNode,
    path: &str,
    flat: &FlatNode,
    heads: &IndexMap<&str, Head<'_>>,
    composed: &[Option<Arc<ComposedNode>>],
) -> Result<(), ComposeError> {
    let resolve = |reference: &str| -> Result<&Arc<ComposedNode>, ComposeError> {
        let (head, tail) = split_head(reference);
        heads
            .get_index_of(head)
            .and_then(|index| composed[index].as_ref())
            .and_then(|head| head.descendant(tail))
            .ok_or_else(|| ComposeError::UnknownReference {
                path: path.to_owned(),
                reference: reference.to_owned(),
            })
    };

    for reference in flat.inherits.values() {
        let inherited = resolve(reference)?;
        for (name, child) in &inherited.children {
            node.children.insert(name.clone(), child.clone());
        }
        for (name, value) in &inherited.attributes {
            node.attributes.insert(name.clone(), value.clone());
        }
    }
    for (name, child) in &flat.children {
        match child {
            Some(reference) => {
                let child = resolve(reference)?.clone();
                node.children.insert(name.clone(), child);
            }
            // shift_remove so a later re-add goes to the end, like a
            // JavaScript Map delete followed by a set.
            None => {
                node.children.shift_remove(name);
            }
        }
    }
    for (name, value) in &flat.attributes {
        node.attributes.insert(name.clone(), value.clone());
    }
    Ok(())
}
