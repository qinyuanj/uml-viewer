//! Port of `love/uml/graph.lua`: shared language-graph utilities.
//! `register`/`lookup` indirection exists in Lua for dynamic loading; in Rust
//! the four scanners are static modules behind [`scan_project`].

use std::collections::HashMap;

use edn::Value;

use crate::files::{Adapter, SourceFile};
use crate::scan;
use crate::value::{encode_key, id_name};
use edn::record::{flag, get, record};

/// A language scanner: file selection plus the project scan.
pub trait Scanner {
    fn accepts(&self, relative: &str, opts: &Value) -> bool;
    /// Whether a directory may contain scannable files; default true.
    fn accepts_directory(&self, relative: &str, opts: &Value) -> bool {
        let _ = (relative, opts);
        true
    }
    /// Explicitly named metadata collected outside the source tree.
    fn metadata_paths(&self, root: &str) -> Vec<String> {
        let _ = root;
        Vec::new()
    }
    fn scan(
        &self,
        root: &str,
        opts: &Value,
        files: &[SourceFile],
        cwd: &str,
    ) -> Result<Value, String>;
}

fn lookup(lang: &Value) -> Result<&'static dyn Scanner, String> {
    let name = match lang {
        Value::Keyword(n) | Value::Symbol(n) | Value::String(n) => n.as_str(),
        _ => "",
    };
    match name {
        "lua" => Ok(&scan::lua::SCANNER),
        "typescript" => Ok(&scan::typescript::SCANNER),
        "rust" => Ok(&scan::rust::SCANNER),
        "python" => Ok(&scan::python::SCANNER),
        other => Err(format!("no LanguageGraph for :{other}")),
    }
}

/// `scan_project(lang, root, opts)`: snapshots the source tree for the
/// language and runs its scanner. `root` is adapter-relative.
pub fn scan_project(
    lang: &Value,
    root: &str,
    opts: &Value,
    adapter: &Adapter,
) -> Result<Value, String> {
    let scanner = lookup(lang)?;
    let (canonical, files) = adapter.snapshot(root, scanner, opts)?;
    let cwd = adapter.root().to_owned();
    scanner.scan(&canonical, opts, &files, &cwd)
}

/// `merge_scans(scans)`: classes dedup by id (a foreign stub yields to the
/// real class; two real classes with one id are an error), edges dedup
/// structurally, and invokes add dependency edges to command owners.
pub fn merge_scans(scans: &[Value]) -> Result<Value, String> {
    let mut classes: Vec<Value> = Vec::new();
    let mut by_id: HashMap<String, usize> = HashMap::new();
    let mut owners: HashMap<String, Value> = HashMap::new();
    for scan in scans {
        for c in edn::record::seq(get(scan, "classes")) {
            let key = encode_key(get(c, "id").unwrap_or(&Value::Nil));
            match by_id.get(&key) {
                None => {
                    classes.push(c.clone());
                    by_id.insert(key, classes.len() - 1);
                }
                Some(&at) => {
                    if !flag(c, "foreign") {
                        if !flag(&classes[at], "foreign") {
                            return Err(format!("duplicate class id: {key}"));
                        }
                        classes[at] = c.clone();
                    }
                }
            }
        }
        if let Some(Value::Map(entries)) = get(scan, "commands") {
            for (id, cmds) in entries {
                for cmd in edn::record::seq(Some(cmds)) {
                    owners.insert(encode_key(cmd), id.clone());
                }
            }
        }
    }
    let mut edges: Vec<Value> = Vec::new();
    let add = |edges: &mut Vec<Value>, e: Value| {
        if !edges.contains(&e) {
            edges.push(e);
        }
    };
    for scan in scans {
        for e in edn::record::seq(get(scan, "edges")) {
            add(&mut edges, e.clone());
        }
    }
    for scan in scans {
        if let Some(Value::Map(entries)) = get(scan, "invokes") {
            for (from, commands) in entries {
                for command in edn::record::seq(Some(commands)) {
                    if let Some(to) = owners.get(&encode_key(command))
                        && to != from
                    {
                        add(
                            &mut edges,
                            record(vec![
                                ("from", from.clone()),
                                ("to", to.clone()),
                                ("kind", Value::keyword("dependency")),
                            ]),
                        );
                    }
                }
            }
        }
    }
    Ok(record(vec![
        ("classes", Value::Vector(classes)),
        ("edges", Value::Vector(edges)),
    ]))
}

fn blank(s: Option<&Value>) -> bool {
    match s {
        _ if !edn::record::truthy(s) => true,
        Some(Value::String(s)) => s
            .trim_matches(|c: char| matches!(c, ' ' | '\t' | '\n' | '\u{b}' | '\u{c}' | '\r'))
            .is_empty(),
        _ => false,
    }
}

fn as_str(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Keyword(n) | Value::Symbol(n) => n.clone(),
        _ => String::new(),
    }
}

/// `id_of(ns, prefix)`: strip a leading `prefix.`; the result is a keyword.
pub fn id_of(ns: &str, prefix: Option<&Value>) -> Value {
    let mut ns = ns.to_owned();
    if let Some(prefix) = prefix.filter(|p| !blank(Some(p))) {
        let prefix = as_str(prefix);
        if ns.starts_with(&format!("{prefix}.")) {
            ns = ns[prefix.len() + 1..].to_owned();
        }
    }
    Value::keyword(&ns)
}

/// `module_name(cid)`: the display name from the last dotted segment.
pub fn module_name(cid: &Value) -> String {
    let segment = {
        let name = id_name(cid);
        let stripped = name.strip_suffix('.').unwrap_or(&name);
        stripped.rsplit('.').next().unwrap_or(stripped).to_owned()
    };
    let mut words = Vec::new();
    for part in segment.split('-') {
        let mut chars = part.chars();
        let Some(first) = chars.next() else { continue };
        let rest: String = chars.collect();
        let rest = if part.chars().any(|c| c.is_ascii_uppercase()) {
            rest
        } else {
            rest.to_lowercase()
        };
        words.push(format!("{}{}", first.to_uppercase(), rest));
    }
    words.concat()
}

/// `relative_path(path, cwd)`: both sides canonicalized by the caller.
pub fn relative_path(path: &str, cwd: &str) -> String {
    let prefix = format!("{cwd}/");
    match path.strip_prefix(&prefix) {
        Some(rest) => rest.to_owned(),
        None => path.to_owned(),
    }
}

/// `file_base(path)`: last segment, allowing a trailing slash.
pub fn file_base(path: &str) -> Option<String> {
    let trimmed = path.trim_end_matches('/');
    trimmed.rsplit('/').next().map(str::to_owned)
}

/// `ns_join(prefix, relative)`.
pub fn ns_join(prefix: &str, relative: &str) -> String {
    if relative.is_empty() {
        return prefix.to_owned();
    }
    if prefix.is_empty() {
        return relative.to_owned();
    }
    format!("{prefix}.{relative}")
}

/// `foreign_class(cid)`.
pub fn foreign_class(cid: &Value) -> Value {
    record(vec![
        ("id", cid.clone()),
        ("name", Value::String(id_name(cid))),
        ("ns", Value::String(id_name(cid))),
        ("foreign", Value::Bool(true)),
    ])
}
