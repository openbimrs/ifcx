//! A library crate's public API, read from its source with `syn`.
//!
//! The summary lists the crate's public modules and every item at the
//! crate root (defined there or re-exported), each with the first paragraph
//! of its doc comment. Re-exports are followed through private modules to
//! the definition, so the summary is what rustdoc shows at the root. An
//! item that cannot be resolved is an error rather than a guess.

use std::path::{Path, PathBuf};

use syn::{Attribute, Expr, ExprLit, Item, Lit, Meta, UseTree, Visibility};

use crate::text::{escape, flatten_rustdoc_links, summary};

/// One public module or root item.
pub(crate) struct Entry {
    pub(crate) name: String,
    pub(crate) kind: &'static str,
    pub(crate) doc: String,
}

/// The public API of the library rooted at `lib`: (modules, root items).
pub(crate) fn summarise(lib: &Path) -> Result<(Vec<Entry>, Vec<Entry>), String> {
    let root = Module::load(lib)?;
    let mut modules = Vec::new();
    let mut items = Vec::new();
    for item in &root.items {
        if !public(item) || cfg_gated(attrs(item)) {
            continue;
        }
        match item {
            Item::Mod(module) => {
                let child = root.child(&module.ident.to_string())?;
                let mut doc = docs(&module.attrs);
                if doc.is_empty() {
                    doc = child.doc.clone();
                }
                modules.push(Entry {
                    name: module.ident.to_string(),
                    kind: "module",
                    doc,
                });
            }
            Item::Use(use_item) => {
                let mut imports = Vec::new();
                flatten_use(&use_item.tree, Vec::new(), &mut imports);
                for (path, name, alias) in imports {
                    let (kind, doc) = resolve_path(&[&root], &path, &name, 0).map_err(|error| {
                        format!("{}: `pub use` of {name}: {error}", lib.display())
                    })?;
                    items.push(Entry {
                        name: alias,
                        kind,
                        doc,
                    });
                }
            }
            other => {
                if let Some((name, kind)) = defined(other) {
                    items.push(Entry {
                        name,
                        kind,
                        doc: docs(attrs(other)),
                    });
                }
            }
        }
    }
    items.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    Ok((modules, items))
}

/// The summary as Markdown tables, items linked to rustdoc's search.
/// `target` makes each link leave the VitePress router: rustdoc is static
/// HTML copied into the site after it is built.
pub(crate) fn markdown(lib_name: &str, modules: &[Entry], items: &[Entry]) -> String {
    let rustdoc = format!("/api/rustdoc/{lib_name}");
    let cell = |doc: &str| escape(&flatten_rustdoc_links(&summary(doc)));
    let mut out = Vec::new();
    if !modules.is_empty() {
        out.push("| Module | Summary |".to_owned());
        out.push("| --- | --- |".to_owned());
        for module in modules {
            out.push(format!(
                "| [`{name}`]({rustdoc}/{name}/index.html){{target=\"_self\"}} | {} |",
                cell(&module.doc),
                name = module.name
            ));
        }
        out.push(String::new());
    }
    if !items.is_empty() {
        out.push("| Item | Kind | Summary |".to_owned());
        out.push("| --- | --- | --- |".to_owned());
        for item in items {
            out.push(format!(
                "| [`{name}`]({rustdoc}/index.html?search={name}){{target=\"_self\"}} | {} | {} |",
                item.kind,
                cell(&item.doc),
                name = item.name
            ));
        }
    }
    out.join("\n").trim().to_owned()
}

/// A parsed module: its items, its `//!` docs, and where its children live.
struct Module {
    items: Vec<Item>,
    doc: String,
    /// Directory holding child module files.
    dir: PathBuf,
}

impl Module {
    fn load(file: &Path) -> Result<Self, String> {
        let source = std::fs::read_to_string(file)
            .map_err(|error| format!("{}: {error}", file.display()))?;
        let parsed =
            syn::parse_file(&source).map_err(|error| format!("{}: {error}", file.display()))?;
        let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let parent = file.parent().unwrap_or(Path::new("")).to_path_buf();
        let dir = if matches!(stem, "lib" | "mod" | "main") {
            parent
        } else {
            parent.join(stem)
        };
        Ok(Self {
            doc: docs(&parsed.attrs),
            items: parsed.items,
            dir,
        })
    }

    /// The child module `name`, declared in this module.
    fn child(&self, name: &str) -> Result<Self, String> {
        for item in &self.items {
            let Item::Mod(module) = item else { continue };
            if module.ident != name {
                continue;
            }
            if let Some((_, items)) = &module.content {
                return Ok(Self {
                    items: items.clone(),
                    doc: docs(&module.attrs),
                    dir: self.dir.join(name),
                });
            }
            for candidate in [
                self.dir.join(format!("{name}.rs")),
                self.dir.join(name).join("mod.rs"),
            ] {
                if candidate.exists() {
                    return Self::load(&candidate);
                }
            }
            return Err(format!(
                "no file for module `{name}` in {}",
                self.dir.display()
            ));
        }
        Err(format!("no module `{name}`"))
    }
}

/// Resolve `name`, imported through `path`, starting in the innermost
/// module of `chain` (the crate root first).
fn resolve_path(
    chain: &[&Module],
    path: &[String],
    name: &str,
    depth: usize,
) -> Result<(&'static str, String), String> {
    if depth > 16 {
        return Err("re-exports nest too deep".to_owned());
    }
    let mut owned: Vec<Module> = Vec::new();
    // Walk the path to the module that holds `name`.
    let mut current: Vec<&Module> = chain.to_vec();
    let mut segments = path.iter().peekable();
    if let Some(first) = segments.peek() {
        match first.as_str() {
            "crate" => {
                current.truncate(1);
                segments.next();
            }
            "self" => {
                segments.next();
            }
            _ => {}
        }
    }
    let segments: Vec<&String> = segments.collect();
    let mut supers = 0;
    let mut names = Vec::new();
    for segment in &segments {
        if segment.as_str() == "super" {
            if names.is_empty() {
                supers += 1;
                continue;
            }
            names.pop();
            continue;
        }
        names.push(segment.as_str());
    }
    for _ in 0..supers {
        if current.len() <= 1 {
            return Err("`super` above the crate root".to_owned());
        }
        current.pop();
    }
    // An external crate: not part of this crate's API to resolve.
    if let Some(first) = names.first() {
        let local = current
            .last()
            .is_some_and(|module| module.child(first).is_ok());
        if !local && current.len() == chain.len() && supers == 0 {
            return Ok(("re-export", String::new()));
        }
    }
    for segment in &names {
        let next = match owned.last() {
            Some(module) => module.child(segment)?,
            None => current.last().ok_or("empty module chain")?.child(segment)?,
        };
        owned.push(next);
    }
    let mut full: Vec<&Module> = current;
    full.extend(owned.iter());
    let module = *full.last().expect("non-empty chain");
    resolve_in(&full, module, name, depth)
}

fn resolve_in(
    chain: &[&Module],
    module: &Module,
    name: &str,
    depth: usize,
) -> Result<(&'static str, String), String> {
    // A definition here wins over a module of the same name (`compose`
    // is both a module and the function it re-exports).
    for item in &module.items {
        if let Some((ident, kind)) = defined(item) {
            if ident == name {
                return Ok((kind, docs(attrs(item))));
            }
        }
    }
    for item in &module.items {
        let Item::Use(use_item) = item else { continue };
        let mut imports = Vec::new();
        flatten_use(&use_item.tree, Vec::new(), &mut imports);
        for (path, target, alias) in imports {
            if alias == name {
                return resolve_path(chain, &path, &target, depth + 1);
            }
        }
    }
    for item in &module.items {
        if let Item::Mod(child) = item {
            if child.ident == name {
                return Ok(("module", module.child(name)?.doc));
            }
        }
    }
    Err(format!("`{name}` not found in {}", module.dir.display()))
}

/// `use a::{b, c::d as e}` → [(["a"], "b", "b"), (["a", "c"], "d", "e")].
fn flatten_use(tree: &UseTree, prefix: Vec<String>, out: &mut Vec<(Vec<String>, String, String)>) {
    match tree {
        UseTree::Path(path) => {
            let mut next = prefix;
            next.push(path.ident.to_string());
            flatten_use(&path.tree, next, out);
        }
        UseTree::Name(name) => {
            let ident = name.ident.to_string();
            if ident == "self" {
                if let Some((last, rest)) = prefix.split_last() {
                    out.push((rest.to_vec(), last.clone(), last.clone()));
                }
            } else {
                out.push((prefix, ident.clone(), ident));
            }
        }
        UseTree::Rename(rename) => {
            out.push((prefix, rename.ident.to_string(), rename.rename.to_string()));
        }
        UseTree::Group(group) => {
            for tree in &group.items {
                flatten_use(tree, prefix.clone(), out);
            }
        }
        UseTree::Glob(_) => {}
    }
}

/// The name and kind of an item that defines something.
fn defined(item: &Item) -> Option<(String, &'static str)> {
    Some(match item {
        Item::Struct(i) => (i.ident.to_string(), "struct"),
        Item::Enum(i) => (i.ident.to_string(), "enum"),
        Item::Fn(i) => (i.sig.ident.to_string(), "function"),
        Item::Trait(i) => (i.ident.to_string(), "trait"),
        Item::Type(i) => (i.ident.to_string(), "type alias"),
        Item::Const(i) => (i.ident.to_string(), "constant"),
        Item::Static(i) => (i.ident.to_string(), "static"),
        Item::Union(i) => (i.ident.to_string(), "union"),
        Item::Macro(i) => (i.ident.as_ref()?.to_string(), "macro"),
        _ => return None,
    })
}

fn public(item: &Item) -> bool {
    let vis = match item {
        Item::Struct(i) => &i.vis,
        Item::Enum(i) => &i.vis,
        Item::Fn(i) => &i.vis,
        Item::Trait(i) => &i.vis,
        Item::Type(i) => &i.vis,
        Item::Const(i) => &i.vis,
        Item::Static(i) => &i.vis,
        Item::Union(i) => &i.vis,
        Item::Mod(i) => &i.vis,
        Item::Use(i) => &i.vis,
        _ => return false,
    };
    matches!(vis, Visibility::Public(_))
}

fn attrs(item: &Item) -> &[Attribute] {
    match item {
        Item::Struct(i) => &i.attrs,
        Item::Enum(i) => &i.attrs,
        Item::Fn(i) => &i.attrs,
        Item::Trait(i) => &i.attrs,
        Item::Type(i) => &i.attrs,
        Item::Const(i) => &i.attrs,
        Item::Static(i) => &i.attrs,
        Item::Union(i) => &i.attrs,
        Item::Mod(i) => &i.attrs,
        Item::Use(i) => &i.attrs,
        Item::Macro(i) => &i.attrs,
        _ => &[],
    }
}

/// `#[cfg(target_arch = ...)]` items exist only for one target: the wasm
/// binding's exports, which its JavaScript API table documents instead.
fn cfg_gated(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident("cfg"))
}

/// The doc comment of an item, `///` or `//!`, as text.
pub(crate) fn docs(attrs: &[Attribute]) -> String {
    let mut lines = Vec::new();
    for attr in attrs {
        let Meta::NameValue(meta) = &attr.meta else {
            continue;
        };
        if !meta.path.is_ident("doc") {
            continue;
        }
        let Expr::Lit(ExprLit {
            lit: Lit::Str(text),
            ..
        }) = &meta.value
        else {
            continue;
        };
        let value = text.value();
        for line in value.lines() {
            lines.push(line.strip_prefix(' ').unwrap_or(line).to_owned());
        }
        if value.is_empty() {
            lines.push(String::new());
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn use_trees_flatten() {
        let item: syn::ItemUse = syn::parse_quote!(
            pub use a::{
                b,
                c::d as e,
                f::{self, g},
                h::*,
            };
        );
        let mut out = Vec::new();
        flatten_use(&item.tree, Vec::new(), &mut out);
        let names: Vec<String> = out
            .iter()
            .map(|(p, n, a)| format!("{}::{n} as {a}", p.join("::")))
            .collect();
        assert_eq!(
            names,
            ["a::b as b", "a::c::d as e", "a::f as f", "a::f::g as g"]
        );
    }
}
