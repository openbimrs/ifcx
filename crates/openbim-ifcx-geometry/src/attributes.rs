//! Read access to one node's attributes, independent of where they come from.

use indexmap::IndexMap;
use openbim_ifcx::{ComposedNode, FlatNode, IfcxNode};
use serde_json::{Map, Value};

/// A node's attributes keyed by namespaced attribute id, as in a file's
/// `schemas` (for example `points::array` or `gltf::material`).
///
/// Values are the attribute values as written, not the flattened
/// `<id>::<component>` keys the upstream viewer builds. A JSON `null` counts
/// as absent, because in IFCX it deletes an attribute when layers merge.
///
/// Implemented for a single [`IfcxNode`], a [`FlatNode`] from
/// [`openbim_ifcx::flatten()`], a [`ComposedNode`] from
/// [`openbim_ifcx::compose()`], plain attribute maps, and references to them.
pub trait Attributes {
    /// The raw value of attribute `id`, or `None` when absent.
    fn raw_attribute(&self, id: &str) -> Option<&Value>;

    /// The value of attribute `id`, or `None` when absent or `null`.
    fn attribute(&self, id: &str) -> Option<&Value> {
        self.raw_attribute(id).filter(|value| !value.is_null())
    }
}

impl Attributes for IndexMap<String, Value> {
    fn raw_attribute(&self, id: &str) -> Option<&Value> {
        self.get(id)
    }
}

impl Attributes for Map<String, Value> {
    fn raw_attribute(&self, id: &str) -> Option<&Value> {
        self.get(id)
    }
}

impl Attributes for IfcxNode {
    fn raw_attribute(&self, id: &str) -> Option<&Value> {
        self.attributes.as_ref()?.get(id)
    }
}

impl Attributes for FlatNode {
    fn raw_attribute(&self, id: &str) -> Option<&Value> {
        self.attributes.get(id).map(|value| &**value)
    }
}

impl Attributes for ComposedNode {
    fn raw_attribute(&self, id: &str) -> Option<&Value> {
        self.attributes.get(id).map(|value| &**value)
    }
}

impl<T: Attributes + ?Sized> Attributes for &T {
    fn raw_attribute(&self, id: &str) -> Option<&Value> {
        (**self).raw_attribute(id)
    }
}
