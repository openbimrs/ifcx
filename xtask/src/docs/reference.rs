//! One generated reference page per crate, the index that lists them, the
//! sidebar data, the install table and the root README's crate table.
//!
//! Every fact on these pages is read from the crate itself: its manifest,
//! its `README.md`, its public API (Rust source, or the binding's
//! declarations), and its `CHANGELOG.md`. Nothing is typed twice, so
//! editing a README or a changelog without regenerating fails `--check`.

use std::collections::BTreeMap;

use super::release::{self, Registry, Release};
use super::{banner, bindings, rust_api, Output};
use crate::text::{demote, publishable, splice, BLOB, TREE};
use crate::workspace::{Crate, Workspace};

/// Everything a crate page shows that is not on the manifest itself.
struct Facts {
    release: Option<Release>,
    registries: Vec<Registry>,
    requires: String,
}

pub(super) fn generate(workspace: &Workspace) -> Result<Vec<Output>, String> {
    let crates = workspace.crates()?;
    let mut facts = BTreeMap::new();
    for krate in &crates {
        facts.insert(
            krate.name.clone(),
            Facts {
                release: release::latest(workspace, krate)?,
                registries: release::registries(workspace, krate)?,
                requires: release::requirement(workspace, krate)?,
            },
        );
    }
    let mut outputs = vec![
        Output::whole(workspace, "docs/reference/index.md", index(&crates, &facts)),
        Output::whole(
            workspace,
            "docs/.vitepress/data/crates.json",
            sidebar(&crates)?,
        ),
        Output::derive(workspace, "docs/guide/install.md", |current| {
            splice(current, "INSTALL:TABLE", &install_table(&crates, &facts))
        })?,
        Output::derive(workspace, "docs/api/index.md", |current| {
            splice(current, "RUSTDOC:LIST", &rustdoc_list(workspace, &crates))
        })?,
        Output::derive(workspace, "README.md", |current| {
            splice(current, "CRATES:TABLE", &readme_table(&crates, &facts))
        })?,
    ];
    for krate in &crates {
        let page = page(workspace, krate, &crates, &facts[&krate.name])?;
        let rel = format!("docs/reference/crates/{}.md", krate.name);
        outputs.push(Output::whole(workspace, &rel, page));
    }
    Ok(outputs)
}

/// How a crate reaches users, in a few words.
fn distribution(krate: &Crate, facts: &Facts) -> String {
    if !facts.registries.is_empty() {
        return facts
            .registries
            .iter()
            .map(|r| format!("[{} `{}`]({})", r.kind, r.package, r.url))
            .collect::<Vec<_>>()
            .join(" · ");
    }
    if krate.publish {
        "not released yet; build from source".to_owned()
    } else {
        "not published on its own (`publish = false`); ships inside the bindings".to_owned()
    }
}

fn released(facts: &Facts) -> String {
    facts.release.as_ref().map_or_else(
        || "not released".to_owned(),
        |r| format!("{} ({})", r.version, r.date),
    )
}

fn index(crates: &[Crate], facts: &BTreeMap<String, Facts>) -> String {
    let mut out = vec![
        "---".to_owned(),
        "editLink: false".to_owned(),
        "---".to_owned(),
        String::new(),
        banner("the crate manifests and changelogs"),
        String::new(),
        "# Crate reference".to_owned(),
        String::new(),
        "Each crate is versioned and released on its own. Every page here is generated \
         from the crate itself: its manifest, its `README.md`, its public API and its \
         `CHANGELOG.md`. The full Rust API is in the [rustdoc](/api/)."
            .to_owned(),
        String::new(),
        "| Crate | Latest release | Distributed as | Description |".to_owned(),
        "| --- | --- | --- | --- |".to_owned(),
    ];
    for krate in crates {
        let facts = &facts[&krate.name];
        out.push(format!(
            "| [`{name}`](./crates/{name}) | {} | {} | {} |",
            released(facts),
            distribution(krate, facts),
            krate.description,
            name = krate.name
        ));
    }
    out.extend([
        String::new(),
        "## How the crates depend on each other".to_owned(),
        String::new(),
        "The bindings reach IFCX only through the binding core; nothing depends on a \
         binding, and `openbim-ifcx` depends on no other crate here \
         ([ADR 0002](/adr/0002-crate-split-by-dependency-weight))."
            .to_owned(),
        String::new(),
        "```mermaid".to_owned(),
        "flowchart BT".to_owned(),
        "  accTitle: Crate dependencies".to_owned(),
        "  accDescr: Each arrow points from a crate to a workspace crate it depends on.".to_owned(),
    ]);
    for krate in crates {
        out.push(format!("  {}[\"{}\"]", node(&krate.name), krate.name));
    }
    for krate in crates {
        for dep in &krate.internal_deps {
            out.push(format!("  {} --> {}", node(&krate.name), node(dep)));
        }
    }
    out.push("```".to_owned());
    out.push(String::new());
    out.join("\n")
}

/// A Mermaid node id for a crate name.
fn node(name: &str) -> String {
    name.replace('-', "_")
}

/// `docs/.vitepress/data/crates.json`, read by the site config for the sidebar.
fn sidebar(crates: &[Crate]) -> Result<String, String> {
    let list: Vec<serde_json::Value> = crates
        .iter()
        .map(|krate| {
            serde_json::json!({
                "name": krate.name,
                "lib": krate.lib.as_ref().map(|(lib, _)| lib.clone()),
            })
        })
        .collect();
    let mut json = serde_json::to_string_pretty(&list).map_err(|error| error.to_string())?;
    json.push('\n');
    Ok(json)
}

/// One row per installable package.
fn install_table(crates: &[Crate], facts: &BTreeMap<String, Facts>) -> String {
    let mut rows = vec![
        "| Language | Package | Latest release | Install | Requires | Reference |".to_owned(),
        "| --- | --- | --- | --- | --- | --- |".to_owned(),
    ];
    for krate in crates {
        let facts = &facts[&krate.name];
        for registry in &facts.registries {
            let language = match registry.kind {
                "npm" => "JavaScript / TypeScript",
                "PyPI" => "Python",
                _ => "Rust",
            };
            rows.push(format!(
                "| {language} | [`{}`]({}) | {} | `{}` | {} | [`{name}`](/reference/crates/{name}) |",
                registry.package,
                registry.url,
                released(facts),
                registry.install(),
                facts.requires,
                name = krate.name
            ));
        }
    }
    rows.join("\n")
}

/// One rustdoc link per Rust library; the bindings document their own API.
fn rustdoc_list(workspace: &Workspace, crates: &[Crate]) -> String {
    crates
        .iter()
        .filter(|krate| {
            release::npm_manifest(workspace, krate).is_none()
                && release::pyproject(workspace, krate).is_none()
        })
        .filter_map(|krate| {
            let (lib, _) = krate.lib.as_ref()?;
            Some(format!(
                "- [`{lib}`](/api/rustdoc/{lib}/index.html){{target=\"_self\"}} ([`{name}`](/reference/crates/{name})): {}",
                krate.description,
                name = krate.name
            ))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The root README's crate table.
fn readme_table(crates: &[Crate], facts: &BTreeMap<String, Facts>) -> String {
    let mut rows = vec![
        "| Crate | Description | Distributed as | Latest release |".to_owned(),
        "| --- | --- | --- | --- |".to_owned(),
    ];
    for krate in crates {
        let facts = &facts[&krate.name];
        rows.push(format!(
            "| [`{name}`](https://openbimrs.github.io/ifcx/reference/crates/{name}) | {} | {} | {} |",
            krate.description,
            distribution(krate, facts),
            released(facts),
            name = krate.name
        ));
    }
    rows.join("\n")
}

fn page(
    workspace: &Workspace,
    krate: &Crate,
    crates: &[Crate],
    facts: &Facts,
) -> Result<String, String> {
    let dir = &krate.dir;
    let mut out = vec![
        "---".to_owned(),
        "editLink: false".to_owned(),
        "---".to_owned(),
        String::new(),
        banner(&format!(
            "`{dir}/Cargo.toml`, `README.md`, `CHANGELOG.md` and its source"
        )),
        String::new(),
        format!("# {}", krate.name),
        String::new(),
        krate.description.clone(),
        String::new(),
        format!("| Crate | `{}` {} on `main` |", krate.name, krate.version),
        "| --- | --- |".to_owned(),
        format!("| Latest release | {} |", released(facts)),
    ];
    out.push(format!(
        "| Distributed as | {} |",
        distribution(krate, facts)
    ));
    for registry in &facts.registries {
        out.push(format!("| Install | `{}` |", registry.install()));
    }
    out.push(format!("| Requires | {} |", facts.requires));
    out.push(format!("| License | {} |", krate.license));
    let mut api = Vec::new();
    if let Some((lib, _)) = &krate.lib {
        // `target` makes the link leave the VitePress router: rustdoc is
        // static HTML copied in after the site build.
        api.push(format!(
            "[rustdoc](/api/rustdoc/{lib}/index.html){{target=\"_self\"}}"
        ));
    }
    if facts.registries.iter().any(|r| r.kind == "crates.io") {
        api.push(format!("[docs.rs](https://docs.rs/{})", krate.name));
    }
    if !api.is_empty() {
        out.push(format!("| API documentation | {} |", api.join(" · ")));
    }
    out.push(format!("| Source | [`{dir}/`]({TREE}/{dir}) |"));

    // The README, without its title (the page has one).
    let readme = super::read(workspace, &format!("{dir}/README.md"))?;
    let readme = match readme.split_once('\n') {
        Some((first, rest)) if first.starts_with("# ") => rest,
        _ => readme.as_str(),
    };
    out.extend([
        String::new(),
        "## About".to_owned(),
        String::new(),
        "From the crate's `README.md`".to_owned()
            + if facts.registries.iter().any(|r| r.kind != "crates.io") {
                ", which is also its registry page:"
            } else {
                ":"
            },
        String::new(),
        demote(&publishable(readme.trim(), dir)),
    ]);

    // The public API: the binding's own surface, or the Rust crate root.
    if release::npm_manifest(workspace, krate).is_some() {
        out.extend([
            String::new(),
            "## JavaScript API".to_owned(),
            String::new(),
            bindings::javascript(workspace)?,
        ]);
    } else if release::pyproject(workspace, krate).is_some() {
        out.extend([
            String::new(),
            "## Python API".to_owned(),
            String::new(),
            bindings::python(workspace)?,
        ]);
    } else if let Some((lib, path)) = &krate.lib {
        let (modules, items) = rust_api::summarise(path)?;
        if !modules.is_empty() || !items.is_empty() {
            out.extend([
                String::new(),
                "## Public API".to_owned(),
                String::new(),
                "The crate root, as rustdoc shows it. Follow a name to its rustdoc entry."
                    .to_owned(),
                String::new(),
                rust_api::markdown(lib, &modules, &items),
            ]);
        }
    }

    if !krate.features.is_empty() {
        out.extend([
            String::new(),
            "## Cargo features".to_owned(),
            String::new(),
            "| Feature | Default | Enables |".to_owned(),
            "| --- | --- | --- |".to_owned(),
        ]);
        for (feature, values) in &krate.features {
            let default = if krate.default_features.contains(feature) {
                "yes"
            } else {
                ""
            };
            let enables: Vec<String> = values.iter().map(|v| format!("`{v}`")).collect();
            out.push(format!(
                "| `{feature}` | {default} | {} |",
                if enables.is_empty() {
                    "—".to_owned()
                } else {
                    enables.join(", ")
                }
            ));
        }
    }
    if !krate.examples.is_empty() {
        out.extend([String::new(), "## Examples".to_owned(), String::new()]);
        for example in &krate.examples {
            out.push(format!(
                "- [`{example}`]({BLOB}/{dir}/examples/{example}.rs): \
                 `cargo run -p {} --example {example}`",
                krate.name
            ));
        }
    }
    let users: Vec<&str> = crates
        .iter()
        .filter(|c| c.internal_deps.contains(&krate.name))
        .map(|c| c.name.as_str())
        .collect();
    if !krate.internal_deps.is_empty() || !users.is_empty() {
        out.extend([
            String::new(),
            "## Within the workspace".to_owned(),
            String::new(),
        ]);
        let list = |names: &[&str]| {
            names
                .iter()
                .map(|n| format!("[`{n}`](./{n})"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        if !krate.internal_deps.is_empty() {
            let deps: Vec<&str> = krate.internal_deps.iter().map(String::as_str).collect();
            out.push(format!("- Depends on {}", list(&deps)));
        }
        if !users.is_empty() {
            out.push(format!("- Used by {}", list(&users)));
        }
    }

    out.extend([String::new(), "## Changes".to_owned(), String::new()]);
    let changelog = super::read(workspace, &format!("{dir}/CHANGELOG.md"))?;
    let unreleased = section(&changelog, "Unreleased");
    if let Some(notes) = unreleased.filter(|n| !n.is_empty()) {
        out.extend([
            "Unreleased, on `main`:".to_owned(),
            String::new(),
            demote(&publishable(&notes, dir)),
            String::new(),
        ]);
    }
    if let Some(release) = &facts.release {
        let notes = section(&changelog, &release.version).unwrap_or_default();
        out.extend([
            format!("Latest release, {} ({}):", release.version, release.date),
            String::new(),
            if notes.is_empty() {
                "No notes.".to_owned()
            } else {
                demote(&publishable(&notes, dir))
            },
            String::new(),
        ]);
    }
    out.push(format!(
        "Full history: [`{dir}/CHANGELOG.md`]({BLOB}/{dir}/CHANGELOG.md), and every \
         release of every crate on the [project changelog](/project/changelog)."
    ));
    out.push(String::new());
    Ok(out.join("\n"))
}

/// The body of one `## [version]` section of a changelog.
fn section(text: &str, version: &str) -> Option<String> {
    let mut body = Vec::new();
    let mut inside = false;
    let mut found = false;
    for line in text.lines() {
        if let Some((heading, _)) = super::changelog::heading(line) {
            inside = heading == version;
            found |= inside;
            continue;
        }
        if inside && !(line.starts_with('[') && line.contains("]: ")) {
            body.push(line);
        }
    }
    found.then(|| body.join("\n").trim().to_owned())
}
