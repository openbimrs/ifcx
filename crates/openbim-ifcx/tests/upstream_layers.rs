//! Opt-in check: build layer stacks for upstream examples that import layers.
//!
//! Needs the `fs` feature and a local checkout of
//! buildingSMART/IFC5-development in `IFCX_UPSTREAM_DIR`; without it the test
//! passes without doing anything. Upstream examples import `https://ifcx.dev/`
//! URIs. This crate never fetches them; set `IFCX_IMPORTS_MIRROR` to a
//! directory laid out as `<host>/<path>` (for example
//! `mirror/ifcx.dev/@openusd.org/usd@v1.ifcx`) to resolve them offline.
//! Unresolved imports are reported, not failed; any other error fails.
//!
//! ```sh
//! IFCX_UPSTREAM_DIR=../IFC5-development IFCX_IMPORTS_MIRROR=../mirror \
//!     cargo test -p openbim-ifcx --features fs --test upstream_layers -- --nocapture
//! ```

#![cfg(feature = "fs")]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use openbim_ifcx::layers::{FsError, FsResolver, LayerError, LayerStackBuilder};
use openbim_ifcx::IfcxFile;

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

#[test]
fn upstream_examples_build_layer_stacks() {
    let Some(dir) = std::env::var_os("IFCX_UPSTREAM_DIR") else {
        eprintln!("IFCX_UPSTREAM_DIR not set; skipping");
        return;
    };
    let mirror = std::env::var_os("IFCX_IMPORTS_MIRROR");
    let mut files = Vec::new();
    ifcx_files(Path::new(&dir), &mut files);
    files.sort();

    let (mut built, mut unresolved, mut failures) = (0, BTreeMap::new(), Vec::new());
    for path in &files {
        let file = IfcxFile::from_json_slice(&std::fs::read(path).unwrap()).unwrap();
        if file.imports.is_empty() {
            continue;
        }
        let mut resolver = FsResolver::new();
        if let Some(mirror) = &mirror {
            resolver = resolver.map_prefix("https://", mirror);
        }
        let name = path
            .strip_prefix(&dir)
            .unwrap_or(path)
            .display()
            .to_string();
        match LayerStackBuilder::new(resolver).build(path.to_str().unwrap()) {
            Ok(stack) => {
                built += 1;
                let federated = stack.federate();
                let undeclared = federated
                    .data
                    .iter()
                    .flat_map(|n| n.attributes.iter().flatten())
                    .filter(|(id, _)| !federated.schemas.contains_key(*id))
                    .count();
                eprintln!(
                    "ok  {} layers, {} schemas, {} nodes, {undeclared} undeclared attribute values  {name}",
                    stack.layers().len(),
                    federated.schemas.len(),
                    federated.data.len(),
                );
            }
            Err(
                LayerError::Missing { uri, .. }
                | LayerError::Resolver {
                    uri,
                    source: FsError::UnsupportedUri(_),
                    ..
                },
            ) => {
                eprintln!("--  unresolved {uri}  {name}");
                *unresolved.entry(uri).or_insert(0) += 1;
            }
            Err(e) => failures.push(format!("{name}: {e}")),
        }
    }
    eprintln!(
        "{built} stacks built, {} unresolved, {} failures",
        unresolved.values().sum::<usize>(),
        failures.len()
    );
    for (uri, count) in &unresolved {
        eprintln!("  unresolved {count:>3}x {uri}");
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
