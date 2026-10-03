//! `cargo run -p xtask -- todo --check`: every marker of unfinished work
//! names the issue that tracks it.
//!
//! A bare `TODO` is a promise nobody owns: it outlives the context that wrote
//! it and cannot be found from the issue tracker. The accepted form is
//! `TODO(#123)`, which ties the marker to a GitHub issue, so closing the issue
//! is also the moment to delete the marker. `FIXME`/`XXX` and the `todo!()` /
//! `unimplemented!()` macros are held to the same rule: the line must carry
//! an issue reference such as `(#123)`.
//!
//! Without `--check` the command lists every marker with its issue, which is
//! how closed-issue leftovers are found.

use std::process::Command;

use crate::workspace::Workspace;

/// File types that can carry a marker; generated and vendored data cannot.
const PATTERNS: &[&str] = &[
    "*.rs", "*.py", "*.sh", "*.mjs", "*.js", "*.ts", "*.c", "*.h", "*.toml", "*.yml", "*.yaml",
];

/// The marker words, built so this file does not match itself.
fn words() -> [String; 3] {
    [
        format!("{}{}", "TO", "DO"),
        format!("{}{}", "FIX", "ME"),
        format!("{}{}", "X", "XX"),
    ]
}

fn macros() -> [String; 2] {
    [format!("{}!(", "todo"), format!("{}!(", "unimplemented")]
}

pub(crate) fn run(check: bool) -> Result<(), String> {
    let workspace = Workspace::load()?;
    let output = Command::new("git")
        .arg("ls-files")
        .args(PATTERNS)
        .current_dir(&workspace.root)
        .output()
        .map_err(|error| format!("git ls-files: {error}"))?;
    let mut markers = Vec::new();
    for file in String::from_utf8_lossy(&output.stdout).lines() {
        let Ok(text) = std::fs::read_to_string(workspace.root.join(file)) else {
            continue;
        };
        for (index, line) in text.lines().enumerate() {
            if let Some(issue) = marker(line) {
                markers.push((
                    format!("{file}:{}", index + 1),
                    issue,
                    line.trim().to_owned(),
                ));
            }
        }
    }
    let untracked: Vec<String> = markers
        .iter()
        .filter(|(_, issue, _)| issue.is_none())
        .map(|(at, _, line)| format!("{at}: {line}"))
        .collect();
    if !check {
        for (at, issue, line) in &markers {
            let issue = issue.map_or_else(|| "untracked".to_owned(), |n| format!("#{n}"));
            println!("{issue:>9}  {at}: {line}");
        }
    }
    if untracked.is_empty() {
        if check {
            println!("every marker names its issue ({} markers)", markers.len());
        }
        Ok(())
    } else {
        Err(format!(
            "{} markers name no issue; write `{}(#N)` with the GitHub issue that tracks it:\n{}",
            untracked.len(),
            words()[0],
            untracked.join("\n")
        ))
    }
}

/// `None` when the line has no marker, `Some(None)` for a marker without an
/// issue, `Some(Some(n))` for one tracked by issue `n`.
fn marker(line: &str) -> Option<Option<u32>> {
    let has_word = words().iter().any(|word| contains_word(line, word));
    let has_macro = macros().iter().any(|m| contains_macro(line, m));
    if !has_word && !has_macro {
        return None;
    }
    Some(issue_reference(line))
}

/// Whether `word` occurs in `line` as a whole upper-case word. A word
/// opening a code span (`` `TODO(#N)` ``) quotes the convention rather than
/// marking work, so it does not count.
fn contains_word(line: &str, word: &str) -> bool {
    line.match_indices(word).any(|(at, _)| {
        let before = line[..at].chars().next_back();
        let after = line[at + word.len()..].chars().next();
        !before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '`')
            && !after.is_some_and(|c| c.is_alphanumeric() || c == '_')
    })
}

/// Whether the macro call `call` (`name!(`) occurs in `line` on its own. As
/// for words, one opening a code span quotes the convention, and one ending
/// a longer name (`my_todo!(`) is a different macro.
fn contains_macro(line: &str, call: &str) -> bool {
    line.match_indices(call).any(|(at, _)| {
        let before = line[..at].chars().next_back();
        !before.is_some_and(|c| c.is_alphanumeric() || c == '_' || c == '`')
    })
}

/// The first `(#N)` on the line.
fn issue_reference(line: &str) -> Option<u32> {
    line.match_indices("(#").find_map(|(at, _)| {
        let digits: String = line[at + 2..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        let close = line[at + 2 + digits.len()..].starts_with(')');
        if close {
            digits.parse().ok()
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers_need_an_issue() {
        let todo = words()[0].clone();
        assert_eq!(marker("let x = 1;"), None);
        assert_eq!(marker(&format!("// {todo}: later")), Some(None));
        assert_eq!(marker(&format!("// {todo}(#42): later")), Some(Some(42)));
        assert_eq!(marker(&format!("// {todo}(#): later")), Some(None));
        assert_eq!(marker(&format!("// {}(\"x\")", macros()[1])), Some(None));
        // Part of a longer identifier is not a marker.
        assert_eq!(marker(&format!("const {todo}_LIST: u8 = 0;")), None);
        assert_eq!(marker("IFCTODOLIST"), None);
        // Quoting the convention is not a marker.
        assert_eq!(marker(&format!("write `{todo}(#N)` instead")), None);
        let unfinished = macros()[0].clone();
        assert_eq!(marker(&format!("the `{unfinished})` macro")), None);
        assert_eq!(marker(&format!("my_{unfinished})")), None);
        assert_eq!(marker(&format!("    {unfinished})")), Some(None));
    }
}
