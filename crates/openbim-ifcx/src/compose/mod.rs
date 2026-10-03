//! Layer composition, following upstream `src/ifcx-core/composition` of
//! buildingSMART/IFC5-development for the `ifcx_alpha` draft.
//!
//! Composition has three phases upstream: federation puts the nodes of all
//! layers into one list in layer order, flattening merges the nodes that
//! share a path ([`flatten`]), and composition expands `inherits` and
//! `children` into a tree.

mod flatten;

pub use flatten::{flatten, FlatNode};
