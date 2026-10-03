//! Text utilities shared by the generators: region splicing and turning
//! repository Markdown into Markdown the docs site can publish.

/// GitHub URL prefix for files on `main`.
pub(crate) const BLOB: &str = "https://github.com/openbimrs/ifcx/blob/main";
/// GitHub URL prefix for directories on `main`.
pub(crate) const TREE: &str = "https://github.com/openbimrs/ifcx/tree/main";

/// Anchors of CONTRIBUTING.md's release section, which the site publishes
/// as its own page.
pub(crate) const RELEASING_ANCHORS: &[&str] = &[
    "releasing-a-crate",
    "rehearsing-a-release",
    "environments-and-trusted-publishing",
];

/// Replace the text between `<!-- {region}:BEGIN -->` and
/// `<!-- {region}:END -->` with `body`.
///
/// Everything outside the markers is prose a human owns and is kept
/// byte-for-byte.
pub(crate) fn splice(text: &str, region: &str, body: &str) -> Result<String, String> {
    let begin = format!("<!-- {region}:BEGIN -->");
    let end = format!("<!-- {region}:END -->");
    let (before, rest) = text
        .split_once(&begin)
        .ok_or_else(|| format!("missing {begin} marker"))?;
    let (_, after) = rest
        .split_once(&end)
        .ok_or_else(|| format!("missing {end} marker"))?;
    Ok(format!("{before}{begin}\n\n{body}\n\n{end}{after}"))
}

/// Repository Markdown read from `dir` (relative to the root, `""` for the
/// root) made publishable on the site:
///
/// - relative links become absolute GitHub links, since the site has no
///   copy of the files they point at (VitePress fails the build on a dead
///   link), except links to `docs/*.md` pages, which become site routes;
/// - `<` and `{{` outside code are escaped, because VitePress parses
///   Markdown as a Vue template and `Vec<T>` in prose would be a component.
pub(crate) fn publishable(markdown: &str, dir: &str) -> String {
    let mut out = Vec::new();
    let mut fence: Option<String> = None;
    for line in markdown.lines() {
        let trimmed = line.trim_start();
        if let Some(marker) = &fence {
            if trimmed.starts_with(marker.as_str()) {
                fence = None;
            }
            out.push(line.to_owned());
            continue;
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            let marker: String = trimmed
                .chars()
                .take_while(|c| *c == '`' || *c == '~')
                .collect();
            fence = Some(marker);
            out.push(line.to_owned());
            continue;
        }
        out.push(escape(&links(line, dir)));
    }
    out.join("\n")
}

/// Rewrite every `[text](target)` on a line; see [`publishable`].
pub(crate) fn links(line: &str, dir: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(open) = rest.find("](") {
        let start = open + 2;
        let Some(close) = rest[start..].find(')') else {
            break;
        };
        let target = &rest[start..start + close];
        out.push_str(&rest[..start]);
        out.push_str(&rewrite(target, dir));
        out.push(')');
        rest = &rest[start + close + 1..];
    }
    out.push_str(rest);
    out
}

fn rewrite(target: &str, dir: &str) -> String {
    if target.contains("://") || target.starts_with('#') || target.starts_with("mailto:") {
        return target.to_owned();
    }
    let (path, anchor) = match target.split_once('#') {
        Some((path, anchor)) => (path, format!("#{anchor}")),
        None => (target, String::new()),
    };
    let resolved = normalise(dir, path);
    // CONTRIBUTING.md is published as two pages; see docs/contributing.rs.
    if resolved == "CONTRIBUTING.md" {
        let releasing = RELEASING_ANCHORS
            .iter()
            .any(|a| anchor.trim_start_matches('#') == *a);
        let page = if releasing {
            "releasing"
        } else {
            "contributing"
        };
        return format!("/project/{page}{anchor}");
    }
    if let Some(page) = resolved
        .strip_prefix("docs/")
        .and_then(|page| page.strip_suffix(".md"))
    {
        return format!("/{page}{anchor}");
    }
    let base = if resolved.ends_with('/') || !resolved.contains('.') {
        TREE
    } else {
        BLOB
    };
    format!("{base}/{}{anchor}", resolved.trim_end_matches('/'))
}

/// `dir` joined with the relative `path`, `..` and `.` resolved.
fn normalise(dir: &str, path: &str) -> String {
    let mut parts: Vec<&str> = dir.split('/').filter(|p| !p.is_empty()).collect();
    for piece in path.split('/') {
        match piece {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            piece => parts.push(piece),
        }
    }
    let mut joined = parts.join("/");
    if path.ends_with('/') {
        joined.push('/');
    }
    joined
}

/// Escape `<` and `{{` outside inline code spans.
pub(crate) fn escape(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_code = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '`' => {
                in_code = !in_code;
                out.push(c);
            }
            // `<https://…>` is an autolink, not a tag.
            '<' if !in_code && !chars.clone().take(4).eq("http".chars()) => out.push_str("&lt;"),
            '{' if !in_code && chars.peek() == Some(&'{') => out.push_str("&#123;"),
            _ => out.push(c),
        }
    }
    out
}

/// Every Markdown heading one level deeper (`#` → `##`), outside fences.
pub(crate) fn demote(markdown: &str) -> String {
    let mut fenced = false;
    markdown
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("```") {
                fenced = !fenced;
            }
            if !fenced && line.starts_with('#') {
                format!("#{line}")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// First paragraph of a doc text, on one line, safe inside a table cell.
pub(crate) fn summary(doc: &str) -> String {
    doc.trim()
        .split("\n\n")
        .next()
        .unwrap_or("")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace('|', "\\|")
}

/// Keep `[text](https://…)` links; reduce rustdoc intra-doc links
/// (`[`Model`]`, `[text](crate::x)`, `[text][ref]`) to their text.
pub(crate) fn flatten_rustdoc_links(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(open) = rest.find('[') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find(']') else {
            out.push_str(&rest[open..]);
            return out;
        };
        let text = &after[..close];
        let tail = &after[close + 1..];
        if let Some(target) = tail.strip_prefix('(') {
            if let Some(end) = target.find(')') {
                let url = &target[..end];
                if url.starts_with("http://") || url.starts_with("https://") {
                    out.push_str(&rest[open..open + 1 + close + 1 + 1 + end + 1]);
                } else {
                    out.push_str(text);
                }
                rest = &target[end + 1..];
                continue;
            }
        }
        if let Some(reference) = tail.strip_prefix('[') {
            if let Some(end) = reference.find(']') {
                out.push_str(text);
                rest = &reference[end + 1..];
                continue;
            }
        }
        out.push_str(text);
        rest = tail;
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splice_keeps_prose() {
        let page = "a\n<!-- R:BEGIN -->\nold\n<!-- R:END -->\nz";
        assert_eq!(
            splice(page, "R", "new").unwrap(),
            "a\n<!-- R:BEGIN -->\n\nnew\n\n<!-- R:END -->\nz"
        );
        assert!(splice("x", "R", "new").is_err());
    }

    #[test]
    fn links_become_absolute_or_routes() {
        assert_eq!(
            links(
                "[a](../../CHANGELOG.md) [b](../../docs/capabilities.md#x)",
                "crates/c"
            ),
            format!("[a]({BLOB}/CHANGELOG.md) [b](/capabilities#x)")
        );
        assert_eq!(
            links("[b](docs/capabilities.md#x)", ""),
            "[b](/capabilities#x)"
        );
        assert_eq!(links("[d](demo/)", ""), format!("[d]({TREE}/demo)"));
        assert_eq!(
            links("[w](https://a.b) [h](#top)", ""),
            "[w](https://a.b) [h](#top)"
        );
    }

    #[test]
    fn fences_are_left_alone_and_prose_escaped() {
        let text = "a Vec<T> and `Vec<T>`\n```rust\nlet v: Vec<u8> = {{x}};\n```";
        assert_eq!(
            publishable(text, ""),
            "a Vec&lt;T> and `Vec<T>`\n```rust\nlet v: Vec<u8> = {{x}};\n```"
        );
    }

    #[test]
    fn rustdoc_links_flatten_and_web_links_stay() {
        assert_eq!(
            flatten_rustdoc_links("See [`Model`] and [the view](crate::View) or [x][y]."),
            "See `Model` and the view or x."
        );
        assert_eq!(
            escape("see <https://a.b> and a<b"),
            "see <https://a.b> and a&lt;b"
        );
        assert_eq!(
            flatten_rustdoc_links("Read [ISO](https://iso.org) now."),
            "Read [ISO](https://iso.org) now."
        );
    }

    #[test]
    fn headings_demote_outside_fences() {
        assert_eq!(
            demote("# A\n```sh\n# c\n```\n## B"),
            "## A\n```sh\n# c\n```\n### B"
        );
    }
}
