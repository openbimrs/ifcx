//! The recorded upstream evidence on `docs/evidence.md`, from
//! `scripts/upstream-drift/baseline.txt`.
//!
//! The baseline is what the weekly drift workflow compares against, so the
//! page shows exactly the results a drift report would be measured from,
//! and the revision they were recorded at. A check the page cannot describe
//! is an error: a new check needs a line here before it can be published.

use super::Output;
use crate::text::splice;
use crate::workspace::Workspace;

const BASELINE: &str = "scripts/upstream-drift/baseline.txt";
const PAGE: &str = "docs/evidence.md";
const UPSTREAM: &str = "https://github.com/buildingSMART/IFC5-development";

/// Every check of `scripts/upstream-drift.sh`, in its order, with what it runs.
const CHECKS: &[(&str, &str)] = &[
    (
        "imports",
        "Fetches every `ifcx.dev` file the examples import into an offline mirror (`scripts/upstream-drift/fetch-imports.py`)",
    ),
    (
        "round-trip",
        "Reads and writes every example, comparing content (`upstream_round_trip`)",
    ),
    (
        "validation",
        "Validates every example against its own and its imported schemas (`upstream_validation`)",
    ),
    (
        "composition",
        "Composes every example, alone or on top of its example folder (`upstream_composition`)",
    ),
    (
        "layers",
        "Builds and validates every import stack against the mirror (`upstream_layers`)",
    ),
    (
        "decode",
        "Decodes every transform, mesh, curve, point cloud and presentation attribute (`upstream_decode`)",
    ),
    (
        "scene",
        "Builds the render scene of every example (`upstream_scene`)",
    ),
    (
        "parity",
        "Compares composed trees with upstream's TypeScript (`scripts/upstream-parity.sh`)",
    ),
    (
        "gltf",
        "Exports fixtures and examples to GLB and runs the Khronos glTF validator (`scripts/gltf-validate.sh`)",
    ),
];

pub(super) fn generate(workspace: &Workspace) -> Result<Output, String> {
    let text = super::read(workspace, BASELINE)?;
    let revision = text
        .lines()
        .filter(|line| line.starts_with('#'))
        .flat_map(|line| line.split(|c: char| !c.is_ascii_hexdigit()))
        .find(|word| word.len() == 40)
        .ok_or_else(|| format!("{BASELINE}: the header names no full upstream commit"))?
        .to_owned();
    let mut rows: Vec<(usize, String, String)> = Vec::new();
    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
    {
        let (check, summary) = line
            .split_once(':')
            .ok_or_else(|| format!("{BASELINE}: `{line}` is not `check: summary`"))?;
        let index = CHECKS
            .iter()
            .position(|(name, _)| *name == check)
            .ok_or_else(|| {
                format!(
                    "{BASELINE}: check `{check}` is not described in xtask/src/docs/evidence.rs"
                )
            })?;
        rows.push((index, check.to_owned(), summary.trim().to_owned()));
    }
    let mut out = vec![
        format!(
            "Recorded against buildingSMART/IFC5-development [`{}`]({UPSTREAM}/commit/{revision}).",
            &revision[..7]
        ),
        String::new(),
        "| Check | What it runs | Recorded result |".to_owned(),
        "| --- | --- | --- |".to_owned(),
    ];
    let mut previous = None;
    for (index, check, summary) in &rows {
        let (name, what) = if previous == Some(*index) {
            (String::new(), String::new())
        } else {
            (format!("`{check}`"), CHECKS[*index].1.to_owned())
        };
        previous = Some(*index);
        out.push(format!(
            "| {name} | {what} | `{}` |",
            summary.replace('|', "\\|")
        ));
    }
    let body = out.join("\n");
    Output::derive(workspace, PAGE, |current| {
        splice(current, "UPSTREAM:BASELINE", &body)
    })
}
