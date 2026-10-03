//! Assemble the project changelog page from every `CHANGELOG.md`.
//!
//! Each crate owns its `CHANGELOG.md` and versions independently, and the
//! repository keeps a root `CHANGELOG.md` for tooling, CI and documentation.
//! The page lists every unreleased entry first, by source, then every
//! release, newest first. Release versions are per crate, so a release is
//! named by crate and version, never by version alone.

use std::cmp::Ordering;

use super::Output;
use crate::text::{demote, publishable, splice};
use crate::workspace::Workspace;

const TARGET: &str = "docs/project/changelog.md";

/// One release section of one changelog.
struct Section {
    version: String,
    date: String,
    body: String,
}

/// A changelog: who owns it, where it lives, and its sections.
struct Source {
    title: String,
    dir: String,
    sections: Vec<Section>,
}

pub(super) fn generate(workspace: &Workspace) -> Result<Output, String> {
    let mut sources = vec![Source {
        title: "Repository (tooling, CI, documentation)".to_owned(),
        dir: String::new(),
        sections: parse(&super::read(workspace, "CHANGELOG.md")?),
    }];
    let crates = workspace.crates()?;
    let missing: Vec<String> = crates
        .iter()
        .flat_map(|krate| {
            ["CHANGELOG.md", "README.md"]
                .into_iter()
                .filter(|file| !workspace.root.join(&krate.dir).join(file).exists())
                .map(|file| format!("{}/{file}", krate.dir))
        })
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "every crate keeps its own README.md and CHANGELOG.md; missing: {}",
            missing.join(", ")
        ));
    }
    for krate in &crates {
        sources.push(Source {
            title: format!("`{}`", krate.name),
            dir: krate.dir.clone(),
            sections: parse(&super::read(
                workspace,
                &format!("{}/CHANGELOG.md", krate.dir),
            )?),
        });
    }

    let mut out: Vec<String> = vec!["## Unreleased".to_owned(), String::new()];
    let mut any = false;
    for source in &sources {
        for section in &source.sections {
            if section.version.eq_ignore_ascii_case("unreleased") && !section.body.is_empty() {
                any = true;
                out.push(format!("### {}", source.title));
                out.push(String::new());
                out.push(demote(&publishable(&section.body, &source.dir)));
                out.push(String::new());
            }
        }
    }
    if !any {
        out.push("Nothing yet.".to_owned());
        out.push(String::new());
    }

    let mut releases: Vec<(&Source, &Section)> = sources
        .iter()
        .flat_map(|source| source.sections.iter().map(move |s| (source, s)))
        .filter(|(_, s)| !s.version.eq_ignore_ascii_case("unreleased"))
        .collect();
    releases.sort_by(|(a_src, a), (b_src, b)| {
        b.date
            .cmp(&a.date)
            .then_with(|| compare_versions(&b.version, &a.version))
            .then_with(|| a_src.title.cmp(&b_src.title))
    });
    out.push("## Releases".to_owned());
    out.push(String::new());
    for (source, section) in releases {
        let date = if section.date.is_empty() {
            String::new()
        } else {
            format!(" ({})", section.date)
        };
        out.push(format!("### {} {}{date}", source.title, section.version));
        out.push(String::new());
        if section.body.is_empty() {
            out.push("No notes.".to_owned());
        } else {
            out.push(demote(&publishable(&section.body, &source.dir)));
        }
        out.push(String::new());
    }
    let body = out.join("\n").trim().to_owned();
    Output::derive(workspace, TARGET, |current| {
        splice(current, "CHANGELOG", &body)
    })
}

/// Split a changelog into release sections.
///
/// Link-reference lines at the foot (`[0.2.0]: https://...`) belong to the
/// source file, not to any release body.
fn parse(text: &str) -> Vec<Section> {
    let mut sections = Vec::new();
    let mut current: Option<(String, String)> = None;
    let mut body: Vec<&str> = Vec::new();
    let mut flush = |current: Option<(String, String)>, body: &mut Vec<&str>| {
        if let Some((version, date)) = current {
            sections.push(Section {
                version,
                date,
                body: body.join("\n").trim().to_owned(),
            });
        }
        body.clear();
    };
    for line in text.lines() {
        if let Some(found) = heading(line) {
            flush(current.take(), &mut body);
            current = Some(found);
            continue;
        }
        if current.is_some() && !is_link_reference(line) {
            body.push(line);
        }
    }
    flush(current, &mut body);
    sections
}

/// `## [0.2.1] - 2026-09-23` or `## [Unreleased]` → (version, date).
pub(crate) fn heading(line: &str) -> Option<(String, String)> {
    let rest = line.strip_prefix("## [")?;
    let close = rest.find(']')?;
    let version = &rest[..close];
    if version.is_empty() {
        return None;
    }
    let tail = rest[close + 1..].trim_end();
    if tail.is_empty() {
        return Some((version.to_owned(), String::new()));
    }
    let date = tail.trim_start().strip_prefix('-')?.trim_start();
    if date.is_empty() || date.contains(char::is_whitespace) {
        return None;
    }
    Some((version.to_owned(), date.to_owned()))
}

/// `[label]: target` at the start of a line.
fn is_link_reference(line: &str) -> bool {
    let Some(rest) = line.strip_prefix('[') else {
        return false;
    };
    let Some(close) = rest.find(']') else {
        return false;
    };
    close > 0 && rest[close + 1..].starts_with(": ")
}

/// Unreleased first, then versions numerically, pieces split on `.`, `-`, `+`.
pub(crate) fn compare_versions(a: &str, b: &str) -> Ordering {
    let key = |version: &str| -> (u8, Vec<u64>) {
        if version.eq_ignore_ascii_case("unreleased") {
            return (1, Vec::new());
        }
        let parts = version
            .split(['.', '-', '+'])
            .map(|piece| piece.parse().unwrap_or(0))
            .collect();
        (0, parts)
    };
    key(a).cmp(&key(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_parse_with_and_without_dates() {
        assert_eq!(
            heading("## [0.2.1] - 2026-09-23"),
            Some(("0.2.1".into(), "2026-09-23".into()))
        );
        assert_eq!(
            heading("## [Unreleased]"),
            Some(("Unreleased".into(), String::new()))
        );
        assert_eq!(heading("## [0.1.0] - a b"), None);
        assert_eq!(heading("### [0.1.0]"), None);
    }

    #[test]
    fn sections_drop_link_references() {
        let text =
            "# C\n\n## [Unreleased]\n\n## [0.1.0] - 2026-10-03\n\n- x\n\n[0.1.0]: https://x\n";
        let sections = parse(text);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].body, "");
        assert_eq!(sections[1].body, "- x");
    }

    #[test]
    fn versions_order_numerically() {
        assert_eq!(compare_versions("Unreleased", "9.9.9"), Ordering::Greater);
        assert_eq!(compare_versions("0.10.0", "0.9.0"), Ordering::Greater);
    }
}
