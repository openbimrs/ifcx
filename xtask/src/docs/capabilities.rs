//! `docs/capabilities.md` is written by hand, since every row needs evidence
//! a human recorded. What is derivable is checked: every crate its table
//! names must be a workspace member, or be marked `(not created)`.

use crate::workspace::Workspace;

const PAGE: &str = "docs/capabilities.md";

pub(super) fn check(workspace: &Workspace) -> Result<(), String> {
    let text = super::read(workspace, PAGE)?;
    let members: Vec<String> = workspace.crates()?.into_iter().map(|c| c.name).collect();
    let mut problems = Vec::new();
    let mut rows = 0;
    for (index, line) in text.lines().enumerate() {
        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
        // | Capability | Status | IFCX draft | Crate | Notes |
        if cells.len() < 7 || cells[1] == "Capability" || cells[1].starts_with("---") {
            continue;
        }
        rows += 1;
        let cell = cells[4];
        for (at, _) in cell.match_indices("`openbim-") {
            let name: String = cell[at + 1..].chars().take_while(|c| *c != '`').collect();
            let planned = cell[at..].contains("(not created)");
            if !members.contains(&name) && !planned {
                problems.push(format!(
                    "{PAGE}:{}: `{name}` is not a workspace crate; mark it `(not created)`",
                    index + 1
                ));
            }
        }
    }
    if rows == 0 {
        problems.push(format!("{PAGE}: no capability rows found"));
    }
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("\n"))
    }
}
