//! Opt-in check against a local checkout of buildingSMART/IFC5-development.
//!
//! Upstream publishes no license, so its files are never committed here. Set
//! `IFCX_UPSTREAM_DIR` to a checkout to run this; without it the test passes
//! without doing anything.
//!
//! ```sh
//! IFCX_UPSTREAM_DIR=../IFC5-development cargo test --release -p openbim-ifcx \
//!     --test upstream_round_trip -- --nocapture
//! ```

use std::path::{Path, PathBuf};
use std::time::Instant;

use openbim_ifcx::IfcxFile;
use serde_json::Value;

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
fn upstream_files_round_trip() {
    let Some(dir) = std::env::var_os("IFCX_UPSTREAM_DIR") else {
        eprintln!("IFCX_UPSTREAM_DIR not set; skipping");
        return;
    };
    let mut files = Vec::new();
    ifcx_files(Path::new(&dir), &mut files);
    files.sort();
    assert!(!files.is_empty(), "no .ifcx files under {dir:?}");

    let mut failures = Vec::new();
    for path in &files {
        let bytes = std::fs::read(path).unwrap();
        let start = Instant::now();
        let file = match IfcxFile::from_json_slice(&bytes) {
            Ok(file) => file,
            Err(e) => {
                failures.push(format!("{}: {e}", path.display()));
                continue;
            }
        };
        let read_time = start.elapsed();
        let written = file.to_json_string().unwrap();
        let original: Value = serde_json::from_slice(&bytes).unwrap();
        let reread: Value = serde_json::from_str(&written).unwrap();
        if reread != original {
            failures.push(format!("{}: round trip changed content", path.display()));
        }
        eprintln!(
            "{:>10} bytes  {:>8.1?}  {}",
            bytes.len(),
            read_time,
            path.strip_prefix(&dir).unwrap_or(path).display()
        );
    }
    eprintln!("{} files, {} failures", files.len(), failures.len());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
