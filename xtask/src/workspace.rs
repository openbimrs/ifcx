//! The workspace as cargo describes it at run time.
//!
//! The root is resolved from `cargo metadata`, never from a compile-time
//! `CARGO_MANIFEST_DIR`, so a worktree sharing a build cache with another
//! checkout never generates that checkout's docs.

use std::collections::BTreeMap;
use std::path::PathBuf;

use cargo_metadata::{DependencyKind, Metadata, MetadataCommand};

/// Workspace members that are tooling rather than crates of the family.
pub(crate) const TOOLING: &[&str] = &["xtask"];

/// The workspace: its root and its packages.
pub(crate) struct Workspace {
    pub(crate) root: PathBuf,
    metadata: Metadata,
}

/// One crate of the family, as its manifest declares it.
pub(crate) struct Crate {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) description: String,
    /// `false` for `publish = false`.
    pub(crate) publish: bool,
    /// The crate directory, relative to the workspace root.
    pub(crate) dir: String,
    pub(crate) license: String,
    /// `rust-version`, the minimum supported Rust.
    pub(crate) rust_version: Option<String>,
    /// `[features]`, without `default`.
    pub(crate) features: BTreeMap<String, Vec<String>>,
    /// `default` feature values.
    pub(crate) default_features: Vec<String>,
    /// Workspace members this crate depends on at run time, sorted.
    pub(crate) internal_deps: Vec<String>,
    /// The library target: its name (the rustdoc directory) and root file.
    pub(crate) lib: Option<(String, PathBuf)>,
    /// Example targets, by name.
    pub(crate) examples: Vec<String>,
}

impl Workspace {
    pub(crate) fn load() -> Result<Self, String> {
        let metadata = MetadataCommand::new()
            .no_deps()
            .exec()
            .map_err(|error| format!("cargo metadata failed: {error}"))?;
        Ok(Self {
            root: metadata.workspace_root.clone().into_std_path_buf(),
            metadata,
        })
    }

    /// Every member except tooling, sorted by dependency order (a crate
    /// after everything it depends on), then by name.
    pub(crate) fn crates(&self) -> Result<Vec<Crate>, String> {
        let members: Vec<String> = self
            .metadata
            .packages
            .iter()
            .map(|package| package.name.to_string())
            .collect();
        let mut out = Vec::new();
        for package in &self.metadata.packages {
            let name = package.name.to_string();
            if TOOLING.contains(&name.as_str()) {
                continue;
            }
            let mut features: BTreeMap<String, Vec<String>> = package
                .features
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            let default_features = features.remove("default").unwrap_or_default();
            let mut internal_deps: Vec<String> = package
                .dependencies
                .iter()
                .filter(|dep| dep.kind == DependencyKind::Normal)
                .map(|dep| dep.name.clone())
                .filter(|dep| members.contains(dep))
                .collect();
            internal_deps.sort();
            internal_deps.dedup();
            let dir = package
                .manifest_path
                .parent()
                .and_then(|dir| dir.strip_prefix(&self.metadata.workspace_root).ok())
                .map(|dir| dir.to_string())
                .ok_or_else(|| format!("{name}: manifest outside the workspace"))?;
            let kinds = |target: &cargo_metadata::Target| {
                target
                    .kind
                    .iter()
                    .map(|kind| kind.to_string())
                    .collect::<Vec<_>>()
            };
            let lib = package
                .targets
                .iter()
                .find(|target| {
                    kinds(target)
                        .iter()
                        .any(|kind| matches!(kind.as_str(), "lib" | "rlib" | "cdylib"))
                })
                .map(|target| {
                    (
                        target.name.replace('-', "_"),
                        target.src_path.clone().into_std_path_buf(),
                    )
                });
            let mut examples: Vec<String> = package
                .targets
                .iter()
                .filter(|target| kinds(target).iter().any(|kind| kind == "example"))
                .map(|target| target.name.clone())
                .collect();
            examples.sort();
            let description = package
                .description
                .clone()
                .ok_or_else(|| format!("{name}: set `description` in Cargo.toml"))?;
            out.push(Crate {
                version: package.version.to_string(),
                description,
                publish: package.publish.as_ref().is_none_or(|to| !to.is_empty()),
                dir,
                license: package.license.clone().unwrap_or_default(),
                rust_version: package.rust_version.as_ref().map(|v| v.to_string()),
                features,
                default_features,
                internal_deps,
                lib,
                examples,
                name,
            });
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        // Dependency order: repeatedly take the crates whose workspace
        // dependencies are all placed. Deterministic, and a cycle is an error.
        let mut ordered: Vec<Crate> = Vec::new();
        while !out.is_empty() {
            let ready = out.iter().position(|krate| {
                krate
                    .internal_deps
                    .iter()
                    .all(|dep| ordered.iter().any(|placed| &placed.name == dep))
            });
            match ready {
                Some(index) => ordered.push(out.remove(index)),
                None => return Err("workspace dependency cycle".to_owned()),
            }
        }
        Ok(ordered)
    }
}
