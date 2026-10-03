//! The ADR index and the sidebar's ADR list, from the ADR files themselves.
//!
//! Adding `docs/adr/NNNN-slug.md` is the only step needed to publish a
//! record: the index table and `docs/.vitepress/data/adrs.json` (which the
//! site config reads for the sidebar) follow from it.

use super::Output;
use crate::text::splice;
use crate::workspace::Workspace;

const DIR: &str = "docs/adr";

struct Adr {
    number: String,
    slug: String,
    title: String,
    status: String,
}

fn records(workspace: &Workspace) -> Result<Vec<Adr>, String> {
    let dir = workspace.root.join(DIR);
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(|error| format!("{DIR}: {error}"))? {
        let path = entry.map_err(|error| format!("{DIR}: {error}"))?.path();
        let Some(slug) = path.file_stem().and_then(|s| s.to_str()).map(str::to_owned) else {
            continue;
        };
        let number: String = slug.chars().take_while(char::is_ascii_digit).collect();
        if number.len() != 4 || path.extension().is_none_or(|e| e != "md") {
            continue;
        }
        let text = std::fs::read_to_string(&path).map_err(|error| format!("{slug}: {error}"))?;
        let title = text
            .lines()
            .find_map(|line| line.strip_prefix("# "))
            .and_then(|heading| {
                heading
                    .split_once(" — ")
                    .or_else(|| heading.split_once(" -- "))
                    .map(|(_, title)| title.trim().to_owned())
            })
            .ok_or_else(|| format!("{slug}: first heading must be `# {number} — Title`"))?;
        let status = text
            .lines()
            .find_map(|line| {
                let line = line.trim_start_matches(['-', ' ']).replace("**", "");
                line.strip_prefix("Status:").map(|s| s.trim().to_owned())
            })
            .ok_or_else(|| format!("{slug}: missing a `- Status:` line"))?;
        out.push(Adr {
            number,
            slug,
            title,
            status,
        });
    }
    out.sort_by(|a, b| a.number.cmp(&b.number));
    Ok(out)
}

/// The table region of `docs/adr/index.md`.
pub(super) fn index(workspace: &Workspace) -> Result<Output, String> {
    let mut rows = vec![
        "| # | Title | Status |".to_owned(),
        "| ---: | --- | --- |".to_owned(),
    ];
    for adr in records(workspace)? {
        rows.push(format!(
            "| [{}](/adr/{}) | {} | {} |",
            adr.number, adr.slug, adr.title, adr.status
        ));
    }
    Output::derive(workspace, "docs/adr/index.md", |current| {
        splice(current, "ADR:INDEX", &rows.join("\n"))
    })
}

/// `docs/.vitepress/data/adrs.json`, read by the site config for the sidebar.
pub(super) fn sidebar(workspace: &Workspace) -> Result<Output, String> {
    let list: Vec<serde_json::Value> = records(workspace)?
        .into_iter()
        .map(|adr| {
            serde_json::json!({
                "number": adr.number,
                "slug": adr.slug,
                "title": adr.title,
                "status": adr.status,
            })
        })
        .collect();
    let mut json = serde_json::to_string_pretty(&list).map_err(|error| error.to_string())?;
    json.push('\n');
    Ok(Output::whole(
        workspace,
        "docs/.vitepress/data/adrs.json",
        json,
    ))
}
