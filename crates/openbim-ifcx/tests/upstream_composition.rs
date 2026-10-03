//! Opt-in composition of every example in a local checkout of
//! buildingSMART/IFC5-development.
//!
//! Upstream publishes no license, so its files are never committed here. Set
//! `IFCX_UPSTREAM_DIR` to a checkout to run this; without it the test passes
//! without doing anything.
//!
//! ```sh
//! IFCX_UPSTREAM_DIR=../IFC5-development cargo test --release -p openbim-ifcx \
//!     --test upstream_composition -- --nocapture
//! ```
//!
//! Each file is composed on its own. Some examples are layers over another
//! example of the same folder (`Tunnel Excavation/02_*` over `01_*`), and
//! their `imports` name only schema files, so the viewer decides the stack.
//! A file that fails with [`ComposeError::UnknownReference`] is composed
//! again on top of every other file of its folder below `examples/`, in
//! sorted order.
//!
//! For each file the test prints the flatten time ([`flatten_owned`], which
//! moves the read nodes instead of copying their attribute values), the
//! compose time, the number of
//! distinct composed nodes, and the number of nodes in the expanded tree
//! under the artificial root, which is what a deep copy per instance would
//! allocate.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use openbim_ifcx::{compose, flatten_owned, ComposeError, ComposedNode, Composition, IfcxFile};

fn ifcx_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            if path
                .file_name()
                .is_some_and(|n| n != ".git" && n != "node_modules")
            {
                ifcx_files(&path, out);
            }
        } else if path.extension().is_some_and(|e| e == "ifcx") {
            out.push(path);
        }
    }
}

fn read(path: &Path) -> IfcxFile {
    let bytes = std::fs::read(path).unwrap();
    IfcxFile::from_json_slice(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// The folder directly below `examples/` that holds `path`.
fn example_folder(path: &Path) -> &Path {
    path.ancestors()
        .find(|dir| dir.parent().and_then(Path::file_name) == Some("examples".as_ref()))
        .unwrap_or_else(|| path.parent().unwrap())
}

/// Composes the data of `files` in order, weakest first. Returns the flatten
/// and compose times.
fn compose_stack(files: &[PathBuf]) -> (Result<Composition, ComposeError>, Duration, Duration) {
    let data: Vec<_> = files.iter().flat_map(|path| read(path).data).collect();
    let start = Instant::now();
    let flat = flatten_owned(data);
    let flattened = Instant::now();
    let composed = compose(&flat);
    (composed, flattened - start, flattened.elapsed())
}

/// (distinct nodes, nodes in the expanded tree) below `root`, without
/// recursion.
fn tree_size(root: &ComposedNode) -> (usize, u64) {
    let mut size: HashMap<*const ComposedNode, u64> = HashMap::new();
    let mut stack: Vec<(&Arc<ComposedNode>, bool)> =
        root.children.values().map(|c| (c, false)).collect();
    while let Some((node, expanded)) = stack.pop() {
        let key = Arc::as_ptr(node);
        if size.contains_key(&key) {
            continue;
        }
        if expanded {
            let below: u64 = node.children.values().map(|c| size[&Arc::as_ptr(c)]).sum();
            size.insert(key, 1 + below);
        } else {
            stack.push((node, true));
            stack.extend(node.children.values().map(|c| (c, false)));
        }
    }
    let total = root
        .children
        .values()
        .map(|c| size[&Arc::as_ptr(c)])
        .sum::<u64>();
    (size.len(), 1 + total)
}

#[test]
fn upstream_examples_compose() {
    let Some(dir) = std::env::var_os("IFCX_UPSTREAM_DIR") else {
        eprintln!("IFCX_UPSTREAM_DIR not set; skipping");
        return;
    };
    let dir = PathBuf::from(dir);
    let mut files = Vec::new();
    ifcx_files(&dir, &mut files);
    files.sort();
    assert!(!files.is_empty(), "no .ifcx files under {dir:?}");

    let (mut alone, mut stacked, mut failures) = (0, 0, Vec::new());
    for path in &files {
        let name = path.strip_prefix(&dir).unwrap_or(path);
        let (mut result, mut flatten_time, mut time) = compose_stack(std::slice::from_ref(path));
        let mut layers = 1;
        if let Err(ComposeError::UnknownReference { .. }) = result {
            let mut stack = Vec::new();
            ifcx_files(example_folder(path), &mut stack);
            stack.sort();
            stack.retain(|other| other != path);
            stack.push(path.clone());
            layers = stack.len();
            (result, flatten_time, time) = compose_stack(&stack);
        }
        match result {
            Ok(composed) => {
                if layers == 1 {
                    alone += 1;
                } else {
                    stacked += 1;
                }
                let (distinct, expanded) = tree_size(&composed.root());
                eprintln!(
                    "{flatten_time:>10.1?} {time:>10.1?}  {layers:>2} layer(s)  {:>3} roots  {distinct:>8} nodes  {expanded:>10} expanded  {}",
                    composed.roots().len(),
                    name.display()
                );
            }
            Err(e) => failures.push(format!("{}: {e}", name.display())),
        }
    }
    eprintln!(
        "{} files: {alone} compose alone, {stacked} on their folder, {} fail",
        files.len(),
        failures.len()
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
