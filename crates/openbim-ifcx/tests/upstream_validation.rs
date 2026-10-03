//! Opt-in validation of every example in a local checkout of
//! buildingSMART/IFC5-development.
//!
//! Upstream publishes no license, so its files are never committed here. Set
//! `IFCX_UPSTREAM_DIR` to a checkout to run this; without it the test passes
//! without doing anything.
//!
//! Most examples take their schemas from imported files hosted on
//! `ifcx.dev`, which are not in the checkout. This crate does no network
//! access, so download them yourself and name the directory with
//! `IFCX_IMPORTS_DIR`; an import is looked up by the last segment of its URI,
//! for example `ifc@v5a.ifcx`. Imported schemas are merged after the file's
//! own, later ones winning, as upstream's `Federate` does. A file whose only
//! failures are attributes without a schema, and which has an import that
//! could not be found, is listed but not counted as invalid.
//!
//! ```sh
//! IFCX_UPSTREAM_DIR=../IFC5-development IFCX_IMPORTS_DIR=../ifcx-imports \
//!     cargo test --release -p openbim-ifcx --test upstream_validation -- --nocapture
//! ```

use std::path::{Path, PathBuf};

use openbim_ifcx::{FailureKind, IfcxFile};

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
    IfcxFile::from_json_slice(&std::fs::read(path).unwrap())
        .unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn upstream_files_validate() {
    let Some(dir) = std::env::var_os("IFCX_UPSTREAM_DIR") else {
        eprintln!("IFCX_UPSTREAM_DIR not set; skipping");
        return;
    };
    let mut files = Vec::new();
    ifcx_files(Path::new(&dir), &mut files);
    files.sort();
    let imports_dir = std::env::var_os("IFCX_IMPORTS_DIR");
    assert!(!files.is_empty(), "no .ifcx files under {dir:?}");

    let (mut valid, mut needs_imports) = (0, 0);
    let mut failures = Vec::new();
    for path in &files {
        let name = path.strip_prefix(&dir).unwrap_or(path).display();
        let mut file = read(path);
        let mut missing_import = false;
        for import in std::mem::take(&mut file.imports) {
            let local = imports_dir
                .as_ref()
                .zip(import.uri.rsplit('/').next())
                .map(|(dir, last)| Path::new(dir).join(last))
                .filter(|p| p.is_file());
            match local {
                Some(local) => file.schemas.extend(read(&local).schemas),
                None => missing_import = true,
            }
        }
        match file.validate() {
            Ok(()) => {
                valid += 1;
                eprintln!("valid          {name}");
            }
            Err(report) => {
                let only_missing = report
                    .failures
                    .iter()
                    .all(|f| f.kind == FailureKind::MissingSchema);
                if only_missing && missing_import {
                    needs_imports += 1;
                    eprintln!(
                        "needs imports  {name} ({} attributes without a local schema)",
                        report.failures.len()
                    );
                } else {
                    eprintln!("INVALID        {name}");
                    failures.push(format!("{name}: {report}"));
                }
            }
        }
    }
    eprintln!(
        "{} files: {valid} valid, {needs_imports} need imported schemas, {} invalid",
        files.len(),
        failures.len()
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
