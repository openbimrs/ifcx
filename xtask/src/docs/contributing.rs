//! The contributing and releasing pages, from the root `CONTRIBUTING.md`,
//! and the viewer guide, from `demo/README.md`.
//!
//! `CONTRIBUTING.md` is the one source: GitHub shows it to contributors,
//! and the site publishes it as two pages, the release procedure on its
//! own. Nothing is retyped, so the pages cannot disagree with the file.

use super::{banner, Output};
use crate::text::{publishable, RELEASING_ANCHORS};
use crate::workspace::Workspace;

const SOURCE: &str = "CONTRIBUTING.md";
const RELEASING: &str = "## Releasing a crate";

pub(super) fn generate(workspace: &Workspace) -> Result<Vec<Output>, String> {
    let text = super::read(workspace, SOURCE)?;
    let body = text
        .split_once('\n')
        .filter(|(first, _)| first.starts_with("# "))
        .map(|(_, rest)| rest)
        .ok_or("CONTRIBUTING.md must start with a `# ` heading")?;
    let (contributing, releasing) = body
        .split_once(RELEASING)
        .ok_or_else(|| format!("CONTRIBUTING.md has no `{RELEASING}` section"))?;
    // The release section runs to the next `## ` heading, if any; whatever
    // follows it stays on the contributing page.
    let (releasing, after) = match releasing.find("\n## ") {
        Some(at) => (&releasing[..at], &releasing[at..]),
        None => (releasing, ""),
    };
    for anchor in RELEASING_ANCHORS {
        let title = anchor.replace('-', " ");
        if !format!("releasing a crate{}", releasing.to_lowercase()).contains(&title) {
            return Err(format!(
                "CONTRIBUTING.md: the release section lost its `{title}` heading; \
                 update RELEASING_ANCHORS in xtask/src/text.rs"
            ));
        }
    }

    let page = |title: &str, content: String| {
        [
            "---".to_owned(),
            "editLink: false".to_owned(),
            "---".to_owned(),
            String::new(),
            banner("`CONTRIBUTING.md`"),
            String::new(),
            format!("# {title}"),
            String::new(),
            publishable(content.trim(), ""),
            String::new(),
        ]
        .join("\n")
    };
    let contributing = format!("{}\n{}", contributing.trim_end(), after);
    // On its own page the release section's subsections move up one level.
    let releasing: String = releasing
        .lines()
        .map(|line| {
            line.strip_prefix('#')
                .filter(|_| line.starts_with("###"))
                .unwrap_or(line)
        })
        .collect::<Vec<_>>()
        .join("\n");
    let viewer = super::read(workspace, "demo/README.md")?;
    let viewer = viewer
        .split_once('\n')
        .filter(|(first, _)| first.starts_with("# "))
        .map(|(_, rest)| rest)
        .ok_or("demo/README.md must start with a `# ` heading")?;
    let viewer = [
        "---".to_owned(),
        "editLink: false".to_owned(),
        "---".to_owned(),
        String::new(),
        banner("`demo/README.md`"),
        String::new(),
        "# The viewer".to_owned(),
        String::new(),
        "::: tip".to_owned(),
        "[Open the viewer](/viewer/){target=\"_self\"}".to_owned(),
        ":::".to_owned(),
        String::new(),
        publishable(viewer.trim(), "demo"),
        String::new(),
    ]
    .join("\n");
    Ok(vec![
        Output::whole(workspace, "docs/guide/viewer.md", viewer),
        Output::whole(
            workspace,
            "docs/project/contributing.md",
            page("Contributing", contributing),
        ),
        Output::whole(
            workspace,
            "docs/project/releasing.md",
            page("Releasing a crate", releasing),
        ),
    ])
}
