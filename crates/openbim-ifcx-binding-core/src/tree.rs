//! The composed tree as nested plain objects.
//!
//! Every node is `{"path", "attributes", "children"}`, the shape of
//! upstream's `PostCompositionNode` with `node` named `path` and of the
//! `compose-json` example of `openbim-ifcx`. The tree is written straight to
//! JSON text, without an intermediate `serde_json::Value`.

use openbim_ifcx::ComposedNode;
use serde::ser::{Serialize, SerializeMap, Serializer};

/// Serialises a composed node and its descendants.
pub(crate) struct Tree<'a>(pub(crate) &'a ComposedNode);

struct Attributes<'a>(&'a ComposedNode);

struct Children<'a>(&'a ComposedNode);

impl Serialize for Tree<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(3))?;
        map.serialize_entry("path", &self.0.path)?;
        map.serialize_entry("attributes", &Attributes(self.0))?;
        map.serialize_entry("children", &Children(self.0))?;
        map.end()
    }
}

impl Serialize for Attributes<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.attributes.len()))?;
        for (id, value) in &self.0.attributes {
            map.serialize_entry(id, &**value)?;
        }
        map.end()
    }
}

impl Serialize for Children<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.children.len()))?;
        for (name, child) in &self.0.children {
            map.serialize_entry(name, &Tree(child))?;
        }
        map.end()
    }
}
