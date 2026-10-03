//! `flatten` + `compose` over node lists: IFCX JSON, or structured node
//! opinions over a small set of paths so that references collide, form
//! cycles and diamonds, and edit inherited children (see the crate docs).
#![no_main]

use libfuzzer_sys::fuzz_target;
use openbim_ifcx::{compose, flatten, ComposeError};
use openbim_ifcx_fuzz::{check_composition, input_nodes};

fuzz_target!(|data: &[u8]| {
    let Some(nodes) = input_nodes(data, false) else {
        return;
    };
    let flat = flatten(&nodes);
    for node in &nodes {
        assert!(flat.contains_key(&node.path), "flatten lost {}", node.path);
    }
    match compose(&flat) {
        Ok(composition) => check_composition(&composition),
        Err(ComposeError::Cycle { cycle }) => assert!(!cycle.is_empty()),
        Err(error) => {
            let _ = error.to_string();
        }
    }
});
