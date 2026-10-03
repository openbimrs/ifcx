//! Flattening: merge every input node that shares a path into one node.
//!
//! This is the first half of composition, upstream's
//! `FlattenCompositionInput` (`src/ifcx-core/composition/compose.ts` in
//! buildingSMART/IFC5-development).

use std::sync::Arc;

use indexmap::IndexMap;
use serde_json::Value;

use crate::model::IfcxNode;

/// All opinions about one path, merged in layer order.
///
/// Upstream calls this a pre-composition node. References are still paths;
/// composition resolves them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FlatNode {
    /// Child name to referenced path. A `None` value survives flattening so
    /// that composition can delete a child inherited from elsewhere.
    pub children: IndexMap<String, Option<String>>,
    /// Inherit name to referenced path. `null` entries are already applied
    /// and removed.
    pub inherits: IndexMap<String, String>,
    /// Attribute values. A JSON `null` is kept as a value, as upstream does;
    /// it does not delete the attribute.
    pub attributes: IndexMap<String, Arc<Value>>,
}

impl FlatNode {
    /// Merges one later opinion into this node.
    ///
    /// For each map, entries of `node` are applied in their order and replace
    /// an existing entry of the same name in place:
    ///
    /// - `children`: the value is stored as written, `None` included;
    /// - `inherits`: `None` removes the entry, a path sets it;
    /// - `attributes`: the value is stored as written, `null` included.
    ///
    /// The node's `path` and unknown fields are not looked at.
    pub fn merge(&mut self, node: &IfcxNode) {
        if let Some(children) = &node.children {
            for (name, child) in children {
                self.children.insert(name.clone(), child.clone());
            }
        }
        if let Some(inherits) = &node.inherits {
            for (name, inherit) in inherits {
                match inherit {
                    // shift_remove so a later re-add goes to the end, like a
                    // JavaScript object delete followed by a set.
                    None => {
                        self.inherits.shift_remove(name);
                    }
                    Some(path) => {
                        self.inherits.insert(name.clone(), path.clone());
                    }
                }
            }
        }
        if let Some(attributes) = &node.attributes {
            for (name, value) in attributes {
                self.attributes
                    .insert(name.clone(), Arc::new(value.clone()));
            }
        }
    }
}

/// Merges input nodes into one [`FlatNode`] per path.
///
/// `nodes` must be in layer order, weakest first: the nodes of one file in
/// file order, and for a layer stack the nodes of each layer after those of
/// the layers it overrides. Later nodes win, as described on
/// [`FlatNode::merge`].
///
/// The result is keyed by path, in order of each path's first appearance.
/// Every map keeps insertion order. Upstream stores these maps in JavaScript
/// objects, which list integer-like keys such as `"2"` first; that ordering
/// quirk is not reproduced, the content is the same.
/// Paths are compared as plain strings, so `a/b` is a node of its own here;
/// composition applies it to child `b` of `a`.
///
/// ```
/// use openbim_ifcx::{flatten, IfcxNode};
///
/// let layers: Vec<IfcxNode> = serde_json::from_str(r#"[
///     {"path": "w", "children": {"Body": "b1"}, "inherits": {"Type": "t"},
///      "attributes": {"x::height": 2.5}},
///     {"path": "w", "children": {"Body": null}, "inherits": {"Type": null},
///      "attributes": {"x::height": 3.0}}
/// ]"#)?;
/// let flat = flatten(&layers);
/// let wall = &flat["w"];
/// assert_eq!(wall.children["Body"], None);
/// assert!(wall.inherits.is_empty());
/// assert_eq!(*wall.attributes["x::height"], 3.0);
/// # Ok::<(), serde_json::Error>(())
/// ```
pub fn flatten<'a, I>(nodes: I) -> IndexMap<String, FlatNode>
where
    I: IntoIterator<Item = &'a IfcxNode>,
{
    let mut flat: IndexMap<String, FlatNode> = IndexMap::new();
    for node in nodes {
        // Look up by &str first so a known path is not cloned again.
        match flat.get_mut(node.path.as_str()) {
            Some(existing) => existing.merge(node),
            None => {
                let mut merged = FlatNode::default();
                merged.merge(node);
                flat.insert(node.path.clone(), merged);
            }
        }
    }
    flat
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn node(value: Value) -> IfcxNode {
        serde_json::from_value(value).unwrap()
    }

    fn keys<V>(map: &IndexMap<String, V>) -> Vec<&str> {
        map.keys().map(String::as_str).collect()
    }

    #[test]
    fn later_entries_replace_in_place() {
        let nodes = [
            node(json!({"path": "a", "children": {"x": "1", "y": "2"},
                        "attributes": {"p": 1, "q": 2}})),
            node(json!({"path": "a", "children": {"x": "3"}, "attributes": {"p": 4}})),
        ];
        let flat = flatten(&nodes);
        let a = &flat["a"];
        assert_eq!(keys(&a.children), ["x", "y"]);
        assert_eq!(a.children["x"].as_deref(), Some("3"));
        assert_eq!(keys(&a.attributes), ["p", "q"]);
        assert_eq!(*a.attributes["p"], json!(4));
    }

    #[test]
    fn null_inherit_removes_and_readd_moves_to_end() {
        let nodes = [
            node(json!({"path": "a", "inherits": {"t": "T", "u": "U"}})),
            node(json!({"path": "a", "inherits": {"t": null}})),
            node(json!({"path": "a", "inherits": {"t": "T2"}})),
        ];
        let flat = flatten(&nodes);
        assert_eq!(keys(&flat["a"].inherits), ["u", "t"]);
        assert_eq!(flat["a"].inherits["t"], "T2");
    }

    #[test]
    fn null_child_and_null_attribute_are_kept() {
        let nodes = [
            node(json!({"path": "a", "children": {"x": "1"}, "attributes": {"p": 1}})),
            node(json!({"path": "a", "children": {"x": null}, "attributes": {"p": null}})),
        ];
        let flat = flatten(&nodes);
        assert_eq!(flat["a"].children["x"], None);
        assert_eq!(*flat["a"].attributes["p"], Value::Null);
    }

    #[test]
    fn paths_keep_first_appearance_order() {
        let nodes = [
            node(json!({"path": "b"})),
            node(json!({"path": "a/x"})),
            node(json!({"path": "a"})),
            node(json!({"path": "b", "attributes": {"p": 1}})),
        ];
        let flat = flatten(&nodes);
        assert_eq!(keys(&flat), ["b", "a/x", "a"]);
        assert_eq!(flat["a"], FlatNode::default());
    }
}
