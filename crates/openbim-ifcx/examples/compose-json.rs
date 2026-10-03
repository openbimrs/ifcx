//! Composes the `data` of one or more IFCX files and prints the composed tree
//! as JSON. `scripts/upstream-parity.sh` compares this output with upstream's
//! TypeScript composition.
//!
//! ```sh
//! cargo run --release -p openbim-ifcx --example compose-json -- base.ifcx overlay.ifcx
//! ```
//!
//! Files are layers in the order given, weakest first: their `data` arrays
//! are concatenated, flattened, and composed, as upstream's `Federate` and
//! `LoadIfcxFile` do. Imports are not resolved; `ifcx_alpha` examples import
//! only schema files, which carry no `data`.
//!
//! On success, stdout holds the artificial root as nested
//! `{"path", "attributes", "children"}` objects, the shape of upstream's
//! `PostCompositionNode` with `node` named `path`. Maps keep this crate's
//! order and numbers are written as read. If composition fails, stdout holds
//! `{"error": {"kind": "cycle" | "unknown-reference", "message": "..."}}`
//! and the exit code is 0. Unreadable input exits with code 1.

use std::io::{BufWriter, Write};
use std::process::ExitCode;

use openbim_ifcx::{compose, flatten_owned, ComposeError, ComposedNode, IfcxFile};
use serde::ser::{Serialize, SerializeMap, Serializer};

/// Serialises a composed node without building an intermediate
/// `serde_json::Value` for the whole tree.
struct Tree<'a>(&'a ComposedNode);

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

fn main() -> ExitCode {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: compose-json <file.ifcx>...");
        return ExitCode::FAILURE;
    }
    let mut data = Vec::new();
    for path in &paths {
        let file = std::fs::read(path)
            .map_err(|e| e.to_string())
            .and_then(|bytes| IfcxFile::from_json_slice(&bytes).map_err(|e| e.to_string()));
        match file {
            Ok(file) => data.extend(file.data),
            Err(e) => {
                eprintln!("{path}: {e}");
                return ExitCode::FAILURE;
            }
        }
    }

    let stdout = std::io::stdout().lock();
    let mut out = BufWriter::new(stdout);
    let written = match compose(&flatten_owned(data)) {
        Ok(composed) => serde_json::to_writer(&mut out, &Tree(&composed.root())),
        Err(e) => {
            let kind = match e {
                ComposeError::Cycle { .. } => "cycle",
                ComposeError::UnknownReference { .. } => "unknown-reference",
                _ => "other",
            };
            let error = serde_json::json!({"error": {"kind": kind, "message": e.to_string()}});
            serde_json::to_writer(&mut out, &error)
        }
    };
    match written.map_err(std::io::Error::from).and_then(|()| {
        out.write_all(b"\n")?;
        out.flush()
    }) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("writing output: {e}");
            ExitCode::FAILURE
        }
    }
}
