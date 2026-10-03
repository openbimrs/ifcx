//! The JavaScript and Python API tables of the binding reference pages.
//!
//! Each table is read from the binding's own declaration of its surface:
//! the `#[wasm_bindgen]` exports and the TypeScript declarations next to
//! them, and the public Python package, read with Python's `ast` (parsed,
//! never imported, so no build is needed). A new export reaches the docs
//! with no edit to any page.

use std::process::Command;

use quote::ToTokens;
use syn::{Attribute, FnArg, ImplItem, Item, Meta, Pat, ReturnType, Type};

use super::rust_api::docs;
use crate::text::{escape, summary};
use crate::workspace::Workspace;

const WASM_API: &str = "crates/openbim-ifcx-wasm/src/api.rs";
const FETCH_IMPORTS: &str = "crates/openbim-ifcx-wasm/js/fetch-imports.d.ts";
const PYTHON: &str = "crates/openbim-ifcx-py/python/openbim_ifcx/__init__.py";

/// Markdown for the `@openbim/ifcx` API.
pub(super) fn javascript(workspace: &Workspace) -> Result<String, String> {
    let source = super::read(workspace, WASM_API)?;
    let file = syn::parse_file(&source).map_err(|error| format!("{WASM_API}: {error}"))?;
    let mut declarations = String::new();
    let mut members = vec![
        "| Member | Throws `IfcxError` | Description |".to_owned(),
        "| --- | --- | --- |".to_owned(),
    ];
    for item in &file.items {
        match item {
            Item::Const(constant)
                if constant.attrs.iter().any(|a| {
                    a.path().is_ident("wasm_bindgen")
                        && a.to_token_stream()
                            .to_string()
                            .contains("typescript_custom_section")
                }) =>
            {
                if let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(text),
                    ..
                }) = &*constant.expr
                {
                    declarations.push_str(&text.value());
                }
            }
            Item::Impl(block)
                if block
                    .attrs
                    .iter()
                    .any(|a| a.path().is_ident("wasm_bindgen")) =>
            {
                let class = block.self_ty.to_token_stream().to_string();
                for member in &block.items {
                    let ImplItem::Fn(function) = member else {
                        continue;
                    };
                    members.push(member_row(&class, function)?);
                }
            }
            _ => {}
        }
    }
    if declarations.is_empty() {
        return Err(format!("{WASM_API}: no typescript_custom_section"));
    }
    declarations.push('\n');
    declarations.push_str(&super::read(workspace, FETCH_IMPORTS)?);
    let declared = typescript_declarations(&declarations);

    let mut functions = vec![
        "| Function | Description |".to_owned(),
        "| --- | --- |".to_owned(),
    ];
    let mut types = vec![
        "| Type | Description |".to_owned(),
        "| --- | --- |".to_owned(),
    ];
    for (kind, name, signature, doc) in &declared {
        let doc = escape(&summary(doc));
        if kind == "function" {
            functions.push(format!("| `{}` | {doc} |", signature.replace('|', "\\|")));
        } else {
            types.push(format!("| `{name}` | {doc} |"));
        }
    }
    Ok([
        "### `IfcxFile`".to_owned(),
        String::new(),
        members.join("\n"),
        String::new(),
        "### Functions".to_owned(),
        String::new(),
        functions.join("\n"),
        String::new(),
        "`fetchImports` is plain JavaScript (`fetch-imports.js`); everything else is the \
         WebAssembly module. Each function takes the layers weakest first."
            .to_owned(),
        String::new(),
        "### Types".to_owned(),
        String::new(),
        types.join("\n"),
    ]
    .join("\n"))
}

/// One `IfcxFile` member as a table row.
fn member_row(class: &str, function: &syn::ImplItemFn) -> Result<String, String> {
    let export = options(&function.attrs);
    let name = option(&export, "js_name").unwrap_or_else(|| function.sig.ident.to_string());
    let receiver = function
        .sig
        .inputs
        .iter()
        .any(|a| matches!(a, FnArg::Receiver(_)));
    let mut params = Vec::new();
    for input in &function.sig.inputs {
        let FnArg::Typed(typed) = input else { continue };
        let param = options(&typed.attrs);
        let ident = match &*typed.pat {
            Pat::Ident(ident) => ident.ident.to_string(),
            _ => "value".to_owned(),
        };
        let (ty, optional) = match option(&param, "unchecked_param_type") {
            Some(ty) => (ty, false),
            None => typescript(&typed.ty)?,
        };
        params.push(format!("{ident}{}: {ty}", if optional { "?" } else { "" }));
    }
    let (returns, throws) = match &function.sig.output {
        ReturnType::Default => ("void".to_owned(), false),
        ReturnType::Type(_, ty) => {
            let (inner, throws) = unwrap_result(ty);
            let ty = match option(&export, "unchecked_return_type") {
                Some(ty) => ty,
                None => typescript(inner)?.0,
            };
            (ty, throws)
        }
    };
    let signature = if flag(&export, "getter") {
        format!("file.{name}: {returns}")
    } else if receiver {
        format!("file.{name}({}): {returns}", params.join(", "))
    } else {
        format!("{class}.{name}({}): {returns}", params.join(", "))
    };
    Ok(format!(
        "| `{}` | {} | {} |",
        signature.replace('|', "\\|"),
        if throws { "yes" } else { "" },
        escape(&summary(&docs(&function.attrs)))
    ))
}

/// `(kind, name, signature, doc)` for every `export` in TypeScript
/// declarations, with the JSDoc block before it.
fn typescript_declarations(text: &str) -> Vec<(String, String, String, String)> {
    let mut out = Vec::new();
    let mut doc = String::new();
    let mut lines = text.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        let nested = line.starts_with([' ', '\t']);
        if trimmed.starts_with("/**") {
            let mut block = vec![trimmed.to_owned()];
            let mut closed = trimmed.ends_with("*/");
            while !closed {
                let Some(next) = lines.next() else { break };
                closed = next.trim().ends_with("*/");
                block.push(next.trim().to_owned());
            }
            // Member docs inside an interface describe the member, not
            // the next export.
            if !nested {
                doc = jsdoc(&block.join("\n"));
            }
            continue;
        }
        if nested || trimmed.is_empty() {
            continue;
        }
        let Some(rest) = trimmed.strip_prefix("export ") else {
            doc.clear();
            continue;
        };
        let (kind, rest) = rest.split_once(' ').unwrap_or((rest, ""));
        let name: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if kind == "function" {
            // A signature may span lines; it ends at the `;`.
            let mut signature = rest.to_owned();
            while !signature.trim_end().ends_with(';') {
                let Some(next) = lines.next() else { break };
                signature.push(' ');
                signature.push_str(next.trim());
            }
            let signature = signature
                .trim_end()
                .trim_end_matches(';')
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
                .replace("( ", "(")
                .replace(", )", ")");
            out.push((kind.to_owned(), name, signature, std::mem::take(&mut doc)));
        } else if matches!(kind, "type" | "interface") {
            out.push((
                kind.to_owned(),
                name,
                String::new(),
                std::mem::take(&mut doc),
            ));
        }
    }
    out
}

/// The text of a `/** … */` block, `*` gutters removed.
fn jsdoc(block: &str) -> String {
    block
        .trim_start_matches("/**")
        .trim_end_matches("*/")
        .lines()
        .map(|line| line.trim().trim_start_matches('*').trim())
        .collect::<Vec<_>>()
        .join("\n")
}

/// `wasm_bindgen(...)` arguments, split at top-level commas.
fn options(attrs: &[Attribute]) -> Vec<String> {
    let mut out = Vec::new();
    for attr in attrs.iter().filter(|a| a.path().is_ident("wasm_bindgen")) {
        let Meta::List(list) = &attr.meta else {
            continue;
        };
        let tokens = list.tokens.to_string();
        let (mut current, mut quoted) = (String::new(), false);
        for c in tokens.chars() {
            match c {
                '"' => {
                    quoted = !quoted;
                    current.push(c);
                }
                ',' if !quoted => out.push(std::mem::take(&mut current).trim().to_owned()),
                _ => current.push(c),
            }
        }
        if !current.trim().is_empty() {
            out.push(current.trim().to_owned());
        }
    }
    out
}

fn flag(options: &[String], name: &str) -> bool {
    options.iter().any(|o| o == name)
}

fn option(options: &[String], name: &str) -> Option<String> {
    options.iter().find_map(|o| {
        let (key, value) = o.split_once('=')?;
        (key.trim() == name).then(|| value.trim().trim_matches('"').to_owned())
    })
}

/// `Result<T, JsValue>` → (`T`, throws).
fn unwrap_result(ty: &Type) -> (&Type, bool) {
    if let Type::Path(path) = ty {
        if let Some(segment) = path.path.segments.last() {
            if segment.ident == "Result" {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                        return (inner, true);
                    }
                }
            }
        }
    }
    (ty, false)
}

/// The TypeScript spelling of a Rust type crossing the wasm boundary, and
/// whether a parameter of it is optional.
fn typescript(ty: &Type) -> Result<(String, bool), String> {
    let rust: String = ty
        .to_token_stream()
        .to_string()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    let (spelling, optional) = match rust.as_str() {
        "&[u8]" | "Uint8Array" => ("Uint8Array", false),
        "&str" | "String" => ("string", false),
        "u32" | "i32" | "usize" | "f64" => ("number", false),
        "bool" => ("boolean", false),
        "Option<bool>" => ("boolean", true),
        "Option<String>" => ("string", true),
        "()" => ("void", false),
        "IfcxFile" => ("IfcxFile", false),
        _ => {
            return Err(format!(
                "{WASM_API}: no TypeScript spelling for `{rust}`; add `unchecked_param_type` / \
                 `unchecked_return_type` on the export or a mapping in xtask/src/docs/bindings.rs"
            ))
        }
    };
    Ok((spelling.to_owned(), optional))
}

/// Prints `{module_doc, members: [{name, kind, params, returns, doc,
/// methods}]}` for every name in `__all__`, in that order.
const PYTHON_SCRIPT: &str = r##"
import ast, json, sys

def ann(node):
    return ast.unparse(node).replace("'", "").replace('"', "")

def params(fn, skip_first):
    a = fn.args
    positional = a.posonlyargs + a.args
    defaults = [None] * (len(positional) - len(a.defaults)) + list(a.defaults)
    pairs = list(zip(positional, defaults))[1 if skip_first else 0:]
    out = []
    for arg, default in pairs:
        text = arg.arg + (": " + ann(arg.annotation) if arg.annotation else "")
        out.append(text + (" = " + ast.unparse(default) if default is not None else ""))
    if a.kwonlyargs:
        out.append("*")
        for arg, default in zip(a.kwonlyargs, a.kw_defaults):
            text = arg.arg + (": " + ann(arg.annotation) if arg.annotation else "")
            out.append(text + (" = " + ast.unparse(default) if default is not None else ""))
    return out

source = open(sys.argv[1]).read()
module = ast.parse(source)
lines = source.splitlines()
exported = []
defs = {}
for node in module.body:
    if isinstance(node, ast.Assign) and any(getattr(t, "id", None) == "__all__" for t in node.targets):
        exported = [ast.literal_eval(e) for e in node.value.elts]
    if isinstance(node, (ast.FunctionDef, ast.ClassDef)):
        defs[node.name] = node
    if isinstance(node, ast.Assign) and isinstance(node.targets[0], ast.Name):
        comment = lines[node.lineno - 2].strip() if node.lineno >= 2 else ""
        defs[node.targets[0].id] = ("alias", ast.unparse(node.value),
                                     comment[3:].strip() if comment.startswith("#:") else "")
    if isinstance(node, ast.ImportFrom):
        for alias in node.names:
            defs.setdefault(alias.asname or alias.name, ("import", node.module, ""))

members = []
for name in exported:
    node = defs.get(name)
    if node is None:
        raise SystemExit(f"__all__ names {name}, which the module does not define")
    if isinstance(node, tuple):
        members.append({"name": name, "kind": node[0], "value": node[1], "doc": node[2]})
    elif isinstance(node, ast.FunctionDef):
        members.append({"name": name, "kind": "function", "params": params(node, False),
                        "returns": ann(node.returns) if node.returns else None,
                        "doc": ast.get_docstring(node) or ""})
    else:
        methods = []
        for fn in node.body:
            if not isinstance(fn, ast.FunctionDef) or fn.name.startswith("_"):
                continue
            decorators = [ast.unparse(d) for d in fn.decorator_list]
            kind = "classmethod" if "classmethod" in decorators else \
                   "property" if "property" in decorators else "method"
            methods.append({"name": fn.name, "kind": kind, "params": params(fn, True),
                            "returns": ann(fn.returns) if fn.returns else None,
                            "doc": ast.get_docstring(fn) or ""})
        members.append({"name": name, "kind": "class", "doc": ast.get_docstring(node) or "",
                        "methods": methods})
print(json.dumps({"members": members}))
"##;

/// Markdown for the `openbim_ifcx` Python API.
pub(super) fn python(workspace: &Workspace) -> Result<String, String> {
    let output = Command::new("python3")
        .arg("-c")
        .arg(PYTHON_SCRIPT)
        .arg(workspace.root.join(PYTHON))
        .output()
        .map_err(|error| format!("python3 is needed to read the Python API: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "reading {PYTHON}: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("reading {PYTHON}: {error}"))?;
    let text = |value: &serde_json::Value, key: &str| {
        value
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned()
    };
    let signature = |owner: &str, member: &serde_json::Value| {
        let name = text(member, "name");
        let params: Vec<&str> = member["params"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|p| p.as_str())
            .collect();
        let returns = member["returns"]
            .as_str()
            .map(|r| format!(" -> {r}"))
            .unwrap_or_default();
        match text(member, "kind").as_str() {
            "classmethod" => format!("{owner}.{name}({}){returns}", params.join(", ")),
            "property" => format!("file.{name}{}", returns.replacen(" ->", ":", 1)),
            "method" => format!("file.{name}({}){returns}", params.join(", ")),
            _ => format!("{name}({}){returns}", params.join(", ")),
        }
    };
    let cell = |doc: &str| {
        escape(&summary(doc).replace("``", "`"))
            .replace(":class:", "")
            .replace(":func:", "")
            .replace(":meth:", "")
    };
    let mut out = Vec::new();
    let members = json["members"].as_array().cloned().unwrap_or_default();
    for member in members.iter().filter(|m| m["kind"] == "class") {
        let name = text(member, "name");
        out.push(format!("### `{name}`"));
        out.push(String::new());
        out.push(cell(&text(member, "doc")));
        out.push(String::new());
        out.push("| Member | Description |".to_owned());
        out.push("| --- | --- |".to_owned());
        for method in member["methods"].as_array().into_iter().flatten() {
            out.push(format!(
                "| `{}` | {} |",
                signature(&name, method).replace('|', "\\|"),
                cell(&text(method, "doc"))
            ));
        }
        out.push(String::new());
    }
    out.push("### Functions and types".to_owned());
    out.push(String::new());
    out.push("| Name | Description |".to_owned());
    out.push("| --- | --- |".to_owned());
    for member in members.iter().filter(|m| m["kind"] != "class") {
        let shown = match text(member, "kind").as_str() {
            "function" => signature("", member),
            "alias" => format!("{} = {}", text(member, "name"), text(member, "value")),
            _ => text(member, "name"),
        };
        let doc = match text(member, "kind").as_str() {
            "import" if text(member, "name") == "IfcxError" => {
                "The exception every failure raises; its `code` is stable.".to_owned()
            }
            _ => cell(&text(member, "doc")),
        };
        out.push(format!("| `{}` | {doc} |", shown.replace('|', "\\|")));
    }
    Ok(out.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typescript_exports_carry_their_jsdoc() {
        let text = "/** A. */\nexport type A = string;\n\n/**\n * F does\n * things.\n */\nexport function f(\n  a: A,\n): void;\n";
        let found = typescript_declarations(text);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].1, "A");
        assert_eq!(found[0].3, "A.");
        assert_eq!(found[1].2, "f(a: A): void");
        assert_eq!(summary(&found[1].3), "F does things.");
    }

    #[test]
    fn unknown_rust_types_are_refused() {
        let ty: Type = syn::parse_quote!(HashMap<u8, u8>);
        assert!(typescript(&ty).is_err());
        let ty: Type = syn::parse_quote!(Option<bool>);
        assert_eq!(typescript(&ty).unwrap(), ("boolean".to_owned(), true));
    }
}
