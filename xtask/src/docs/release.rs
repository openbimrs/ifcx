//! What each crate has released, and where it can be installed from.
//!
//! The released version is the newest dated section of the crate's own
//! `CHANGELOG.md`. The release commit is what dates that section, so this is
//! a property of the commit, not of the moment the docs are built: deriving
//! it from git tags or registries instead would turn `main`'s docs stale the
//! moment a tag is pushed.

use super::changelog::{compare_versions, heading};
use crate::workspace::{Crate, Workspace};

/// The newest release a crate's changelog records.
pub(crate) struct Release {
    pub(crate) version: String,
    pub(crate) date: String,
}

/// A registry a crate is published to.
pub(crate) struct Registry {
    /// `crates.io`, `npm` or `PyPI`.
    pub(crate) kind: &'static str,
    /// The package name there, which need not equal the crate name.
    pub(crate) package: String,
    pub(crate) url: String,
}

impl Registry {
    /// The command that installs the package.
    pub(crate) fn install(&self) -> String {
        match self.kind {
            "crates.io" => format!("cargo add {}", self.package),
            "npm" => format!("npm install {}", self.package),
            _ => format!("pip install {}", self.package),
        }
    }
}

/// The newest dated `## [x.y.z] - date` section, if any.
pub(crate) fn latest(workspace: &Workspace, krate: &Crate) -> Result<Option<Release>, String> {
    let text = super::read(workspace, &format!("{}/CHANGELOG.md", krate.dir))?;
    let newest = text
        .lines()
        .filter_map(heading)
        .filter(|(version, date)| !date.is_empty() && !version.eq_ignore_ascii_case("unreleased"))
        .max_by(|a, b| compare_versions(&a.0, &b.0));
    Ok(newest.map(|(version, date)| Release { version, date }))
}

/// Registries the crate is published to: crates.io unless `publish = false`,
/// plus npm and PyPI when their manifests exist. These are the same files
/// the release workflow publishes from (`scripts/release-crate.py`).
///
/// Empty for a crate that has never been released: a manifest that *could*
/// publish is not a package anyone can install, and a link to it would 404.
pub(crate) fn registries(workspace: &Workspace, krate: &Crate) -> Result<Vec<Registry>, String> {
    let mut out = Vec::new();
    if latest(workspace, krate)?.is_none() {
        return Ok(out);
    }
    if krate.publish {
        out.push(Registry {
            kind: "crates.io",
            package: krate.name.clone(),
            url: format!("https://crates.io/crates/{}", krate.name),
        });
    }
    if let Some(name) = npm_manifest(workspace, krate)
        .and_then(|json| json.get("name")?.as_str().map(str::to_owned))
    {
        out.push(Registry {
            kind: "npm",
            url: format!("https://www.npmjs.com/package/{name}"),
            package: name,
        });
    }
    if let Some(name) = pyproject(workspace, krate).and_then(|text| project_value(&text, "name")) {
        out.push(Registry {
            kind: "PyPI",
            url: format!("https://pypi.org/project/{name}/"),
            package: name,
        });
    }
    Ok(out)
}

/// `npm/package.json` of the crate, if it ships to npm.
pub(crate) fn npm_manifest(workspace: &Workspace, krate: &Crate) -> Option<serde_json::Value> {
    std::fs::read_to_string(workspace.root.join(&krate.dir).join("npm/package.json"))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
}

/// `pyproject.toml` of the crate, if it ships to PyPI.
pub(crate) fn pyproject(workspace: &Workspace, krate: &Crate) -> Option<String> {
    std::fs::read_to_string(workspace.root.join(&krate.dir).join("pyproject.toml")).ok()
}

/// The toolchain floor a package declares for itself.
pub(crate) fn requirement(workspace: &Workspace, krate: &Crate) -> Result<String, String> {
    if let Some(manifest) = npm_manifest(workspace, krate) {
        let node = manifest["engines"]["node"]
            .as_str()
            .ok_or_else(|| format!("{}/npm/package.json declares no `engines.node`", krate.dir))?;
        return Ok(format!("Node `{node}`, or a current browser"));
    }
    if let Some(text) = pyproject(workspace, krate) {
        let floor = project_value(&text, "requires-python")
            .ok_or_else(|| format!("{}/pyproject.toml declares no `requires-python`", krate.dir))?;
        return Ok(format!("Python `{floor}`"));
    }
    match &krate.rust_version {
        Some(version) => Ok(format!("Rust `{version}`")),
        None => Err(format!("{}: set `rust-version`", krate.name)),
    }
}

/// `key = "…"` in the `[project]` table of a `pyproject.toml`.
fn project_value(text: &str, key: &str) -> Option<String> {
    let mut in_project = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_project = line == "[project]";
            continue;
        }
        if !in_project {
            continue;
        }
        if let Some(value) = line.strip_prefix(key) {
            let Some(value) = value.trim_start().strip_prefix('=') else {
                continue;
            };
            return Some(value.trim().trim_matches('"').to_owned());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pyproject_values_are_read_from_project_table() {
        let text = "[build-system]\nname = \"x\"\n[project]\nname = \"openbim-ifcx\"\nrequires-python = \">=3.9\"\n";
        assert_eq!(project_value(text, "name").as_deref(), Some("openbim-ifcx"));
        assert_eq!(
            project_value(text, "requires-python").as_deref(),
            Some(">=3.9")
        );
        assert_eq!(project_value("[tool]\nname = \"y\"\n", "name"), None);
    }
}
