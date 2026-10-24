//! Port of `love/uml/lua_language/graph_lua.lua`: a pure Lua module scanner
//! over caller-supplied canonical source snapshots. Scanner limitations are
//! preserved (the goldens encode them); positions are 1-based like Lua.

use edn::Value;

use crate::graph::{self, Scanner};
use crate::text::{self, at, is_alpha_, is_space, is_word};
use crate::value::get_truthy as get;
use edn::record::{flag, record, seq};

const SKIP_DIRS: &[&str] = &[
    "lua_modules",
    "spec",
    "tests",
    "test",
    "dist",
    "build",
    "target",
    "out",
    "coverage",
];

fn ident(c: Option<u8>) -> bool {
    c.is_some_and(is_word)
}

/// `keyword(s, i, w)`: literal `w` at 1-based `i` with non-identifier flanks.
fn keyword(s: &[u8], i: usize, w: &[u8]) -> bool {
    text::sub(s, i as i64, i as i64 + w.len() as i64 - 1) == w
        && !ident(at(s, i.wrapping_sub(1)))
        && !ident(at(s, i + w.len()))
}

/// `long_end(s, i)`: a long bracket `[=*[` at `i`; returns one past `]=*]`.
fn long_end(s: &[u8], i: usize) -> Option<usize> {
    if at(s, i) != Some(b'[') {
        return None;
    }
    let mut j = i + 1;
    while at(s, j) == Some(b'=') {
        j += 1;
    }
    if at(s, j) != Some(b'[') {
        return None;
    }
    let eq_len = j - (i + 1);
    let mut close = vec![b']'];
    close.extend(std::iter::repeat_n(b'=', eq_len));
    close.push(b']');
    let start = i + eq_len + 2;
    match text::find_plain(s, &close, start) {
        Some(b) => Some(b + close.len()),
        None => Some(s.len() + 1),
    }
}

fn in_hole(holes: &[(usize, usize)], i: usize) -> bool {
    holes.iter().any(|&(a, b)| i >= a && i < b)
}

/// `mask(source)`: comments and long brackets blanked (newlines kept);
/// quoted strings kept with their interior recorded as a hole.
fn mask(source: &[u8]) -> (Vec<u8>, Vec<(usize, usize)>) {
    let mut out = Vec::new();
    let mut holes = Vec::new();
    let mut i = 1;
    while i <= source.len() {
        let c = at(source, i).unwrap();
        let mut finish = None;
        let mut quoted = false;
        if at(source, i) == Some(b'-') && at(source, i + 1) == Some(b'-') {
            finish = Some(
                long_end(source, i + 2)
                    .or_else(|| text::find_plain(source, b"\n", i))
                    .unwrap_or(source.len() + 1),
            );
        } else if c == b'"' || c == b'\'' {
            finish = Some(graph::scan_quoted(source, i, c, None));
            quoted = true;
        } else if c == b'[' {
            finish = long_end(source, i);
        }
        if let Some(finish) = finish {
            let text = &source[i - 1..finish - 1];
            if quoted {
                out.extend_from_slice(text);
                holes.push((i + 1, (source.len() + 1).min(finish - 1)));
            } else {
                out.extend(text.iter().map(|&c| if c == b'\n' { c } else { b' ' }));
            }
            i = finish;
        } else {
            out.push(c);
            i += 1;
        }
    }
    (out, holes)
}

fn strip_spaces(s: &[u8]) -> usize {
    s.iter().take_while(|&&c| is_space(c)).count()
}

/// `[%a_][%w_%.:]*` capture starting at 0 (byte offset into the slice).
fn capture_dotted(s: &[u8]) -> Option<usize> {
    if !s.first().is_some_and(|&c| is_alpha_(c)) {
        return None;
    }
    let len = s
        .iter()
        .take_while(|&&c| is_word(c) || c == b'.' || c == b':')
        .count();
    Some(len)
}

/// `^local%s+function%s+([%a_][%w_%.:]*)`.
fn match_local_function(rest: &[u8]) -> Option<Vec<u8>> {
    let s = rest.strip_prefix(b"local")?;
    let n = strip_spaces(s);
    if n == 0 {
        return None;
    }
    let s = s[n..].strip_prefix(b"function")?;
    let n = strip_spaces(s);
    if n == 0 {
        return None;
    }
    let len = capture_dotted(&s[n..])?;
    Some(s[n..n + len].to_vec())
}

/// `^local%s+([%a_][%w_%.:]*)%s*=%s*function%f[^%w_]`.
fn match_local_assign_function(rest: &[u8]) -> Option<Vec<u8>> {
    let s = rest.strip_prefix(b"local")?;
    let n = strip_spaces(s);
    if n == 0 {
        return None;
    }
    let s = &s[n..];
    let len = capture_dotted(s)?;
    let captured = s[..len].to_vec();
    let s = &s[len..];
    let s = &s[strip_spaces(s)..];
    let s = s.strip_prefix(b"=")?;
    let s = &s[strip_spaces(s)..];
    let s = s.strip_prefix(b"function")?;
    if s.first().is_some_and(|&c| is_word(c)) {
        return None;
    }
    Some(captured)
}

/// `^function%s+([%a_][%w_%.:]*)`.
fn match_function(rest: &[u8]) -> Option<Vec<u8>> {
    let s = rest.strip_prefix(b"function")?;
    let n = strip_spaces(s);
    if n == 0 {
        return None;
    }
    let len = capture_dotted(&s[n..])?;
    Some(s[n..n + len].to_vec())
}

/// `^([%a_][%w_%.:]*)%s*=%s*function%f[^%w_]`.
fn match_assign_function(rest: &[u8]) -> Option<Vec<u8>> {
    let len = capture_dotted(rest)?;
    let s = &rest[len..];
    let s = &s[strip_spaces(s)..];
    let s = s.strip_prefix(b"=")?;
    let s = &s[strip_spaces(s)..];
    let s = s.strip_prefix(b"function")?;
    if s.first().is_some_and(|&c| is_word(c)) {
        return None;
    }
    Some(rest[..len].to_vec())
}

/// `declaration(rest, kw)`.
fn declaration(rest: &[u8], kw: Option<&str>) -> (Option<Vec<u8>>, bool) {
    match kw {
        Some("local") => (
            match_local_function(rest).or_else(|| match_local_assign_function(rest)),
            true,
        ),
        Some("function") => (match_function(rest), false),
        None => (match_assign_function(rest), false),
        _ => (None, false),
    }
}

fn rest_of_line(s: &[u8]) -> &[u8] {
    let end = s.iter().position(|&c| c == b'\n').unwrap_or(s.len());
    &s[..end]
}

/// `[ \t]*` prefix skip.
fn space_tab_skip(s: &[u8]) -> &[u8] {
    &s[s.iter().take_while(|&&c| c == b' ' || c == b'\t').count()..]
}

fn statement_before(text: &[u8], i: usize) -> Vec<u8> {
    let before = &text[..i - 1];
    let start = before
        .iter()
        .rposition(|&c| c == b'\n' || c == b';')
        .map_or(0, |p| p + 1);
    before[start..].to_vec()
}

/// `statement:match('local%s*$')`.
fn ends_with_local(s: &[u8]) -> bool {
    let end = s.iter().rposition(|&c| !is_space(c)).map_or(0, |p| p + 1);
    s[..end].ends_with(b"local")
}

/// The `local X = require "mod"` scan: returns (start, end, binding, module).
fn find_binding(text: &[u8], pos: usize) -> Option<(usize, usize, Vec<u8>, Vec<u8>)> {
    for i in pos..=text.len() {
        if let Some((b, binding, module)) = match_binding_at(text, i) {
            return Some((i, b, binding, module));
        }
    }
    None
}

/// `local%s+([%a_][%w_]*)%s*=%s*require%s*%(?%s*["']([%a_][%w_%.]*)` at `i`.
/// Returns (end, binding, module) with `end` 1-based inclusive like Lua's `b`.
fn match_binding_at(t: &[u8], i: usize) -> Option<(usize, Vec<u8>, Vec<u8>)> {
    let mut p = i - 1;
    if !t.get(p..)?.starts_with(b"local") {
        return None;
    }
    p += 5;
    let spaces = t[p..].iter().take_while(|&&c| is_space(c)).count();
    if spaces == 0 {
        return None;
    }
    p += spaces;
    if !t.get(p).is_some_and(|&c| is_alpha_(c)) {
        return None;
    }
    let binding_start = p;
    while t.get(p).is_some_and(|&c| is_word(c)) {
        p += 1;
    }
    let binding = t[binding_start..p].to_vec();
    p += t[p..].iter().take_while(|&&c| is_space(c)).count();
    if t.get(p) != Some(&b'=') {
        return None;
    }
    p += 1;
    p += t[p..].iter().take_while(|&&c| is_space(c)).count();
    if !t[p..].starts_with(b"require") {
        return None;
    }
    p += 7;
    p += t[p..].iter().take_while(|&&c| is_space(c)).count();
    if t.get(p) == Some(&b'(') {
        p += 1;
    }
    p += t[p..].iter().take_while(|&&c| is_space(c)).count();
    if !matches!(t.get(p), Some(b'"') | Some(b'\'')) {
        return None;
    }
    p += 1;
    if !t.get(p).is_some_and(|&c| is_alpha_(c)) {
        return None;
    }
    let module_start = p;
    while t.get(p).is_some_and(|&c| is_word(c) || c == b'.') {
        p += 1;
    }
    let module = t[module_start..p].to_vec();
    Some((p, binding, module))
}

/// `read_module(source)`: requires, definitions and setmetatable bases.
pub fn read_module(source: &[u8]) -> Value {
    let (text, holes) = mask(source);
    let mut requires = Vec::new();
    let mut defns = Vec::new();
    let mut bases: Vec<Value> = Vec::new();
    let mut bindings: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
    let mut base_names: Vec<Vec<u8>> = Vec::new();
    let mut depth: i64 = 0;
    for i in 1..=text.len() {
        if in_hole(&holes, i) {
            continue;
        }
        if keyword(&text, i, b"require") {
            let after = text::sub(&text, (i + 7) as i64, text.len() as i64);
            let after = space_tab_skip(after);
            let mut tail = rest_of_line(after);
            if tail.first() == Some(&b'(') {
                tail = rest_of_line(space_tab_skip(&tail[1..]));
            }
            let quote = tail.first().copied();
            if quote == Some(b'"') || quote == Some(b'\'') {
                let s = &tail[1..];
                if s.first().is_some_and(|&c| is_alpha_(c)) {
                    let len = s.iter().take_while(|&&c| is_word(c) || c == b'.').count();
                    requires.push(Value::String(text::text(&s[..len])));
                }
            }
        }
        let c = at(&text, i);
        let prev = at(&text, i.wrapping_sub(1));
        if ident(c) && !ident(prev) && prev != Some(b'.') && prev != Some(b':') {
            let mut kw_found = None;
            for word in ["function", "local", "if", "do", "repeat", "end", "until"] {
                if keyword(&text, i, word.as_bytes()) {
                    kw_found = Some(word);
                    break;
                }
            }
            let statement = statement_before(&text, i);
            if kw_found == Some("local") || !ends_with_local(&statement) {
                let (n, loc) = declaration(text::sub(&text, i as i64, text.len() as i64), kw_found);
                if let Some(n) = n {
                    defns.push(record(vec![
                        ("name", Value::String(text::text(&n))),
                        ("local", Value::Bool(loc)),
                        ("nested", Value::Bool(depth > 0)),
                    ]));
                }
            }
            if depth == 0 && keyword(&text, i, b"setmetatable") {
                let rest = text::sub(&text, i as i64, text.len() as i64);
                if let Some(tail) = match_setmetatable(rest)
                    && let Some(base) = match_base(&tail)
                {
                    base_names.push(base);
                }
            }
            depth = (depth
                + match kw_found {
                    Some("function") | Some("if") | Some("do") | Some("repeat") => 1,
                    Some("end") | Some("until") => -1,
                    _ => 0,
                })
            .max(0);
        }
    }
    let mut pos = 1;
    while let Some((a, b, binding, module)) = find_binding(&text, pos) {
        if !in_hole(&holes, a) {
            bindings.push((binding, module));
        }
        pos = b + 1;
    }
    for b in &base_names {
        if let Some((_, module)) = bindings.iter().find(|(name, _)| name == b) {
            let v = Value::String(text::text(module));
            if !bases.contains(&v) {
                bases.push(v);
            }
        }
    }
    record(vec![
        ("requires", Value::Vector(requires)),
        ("defns", Value::Vector(defns)),
        ("bases", Value::Vector(bases)),
    ])
}

/// `text:sub(i):match('^setmetatable%s*%(%s*(.*)')`.
fn match_setmetatable(rest: &[u8]) -> Option<Vec<u8>> {
    let s = rest.strip_prefix(b"setmetatable")?;
    let s = &s[strip_spaces(s)..];
    let s = s.strip_prefix(b"(")?;
    let s = &s[strip_spaces(s)..];
    Some(rest_of_line(s).to_vec())
}

/// The base-name alternatives after the first `setmetatable` argument.
fn match_base(tail: &[u8]) -> Option<Vec<u8>> {
    // `^%{%s*%}%s*,%s*(.*)` or `^[%a_][%w_%.]*%s*,%s*(.*)`
    let rest: Option<&[u8]> = if tail.first() == Some(&b'{') {
        let s = &tail[1..];
        let s = &s[strip_spaces(s)..];
        s.strip_prefix(b"}").map(|s| {
            let s = &s[strip_spaces(s)..];
            s.strip_prefix(b",")
                .map(|s| rest_of_line(&s[strip_spaces(s)..]))
        })?
    } else if tail.first().is_some_and(|&c| is_alpha_(c)) {
        let len = tail
            .iter()
            .take_while(|&&c| is_word(c) || c == b'.')
            .count();
        let s = &tail[len..];
        let s = &s[strip_spaces(s)..];
        s.strip_prefix(b",")
            .map(|s| rest_of_line(&s[strip_spaces(s)..]))
    } else {
        None
    };
    let rest = rest?;
    // `^%{%s*__index%s*=%s*([%a_][%w_]*)%s*%}%s*%)` or `^([%a_][%w_]*)%s*%)`
    if rest.first() == Some(&b'{') {
        let s = &rest[1..];
        let s = &s[strip_spaces(s)..];
        if let Some(s) = s.strip_prefix(b"__index") {
            let s = &s[strip_spaces(s)..];
            if let Some(s) = s.strip_prefix(b"=") {
                let s = &s[strip_spaces(s)..];
                if s.first().is_some_and(|&c| is_alpha_(c)) {
                    let len = s.iter().take_while(|&&c| is_word(c)).count();
                    let base = s[..len].to_vec();
                    let s = &s[len..];
                    let s = &s[strip_spaces(s)..];
                    if let Some(s) = s.strip_prefix(b"}") {
                        let s = &s[strip_spaces(s)..];
                        if s.first() == Some(&b')') {
                            return Some(base);
                        }
                    }
                }
            }
        }
        None
    } else if rest.first().is_some_and(|&c| is_alpha_(c)) {
        let len = rest.iter().take_while(|&&c| is_word(c)).count();
        let base = rest[..len].to_vec();
        let s = &rest[len..];
        let s = &s[strip_spaces(s)..];
        if s.first() == Some(&b')') {
            return Some(base);
        }
        None
    } else {
        None
    }
}

/// `module_relative(path, root)`.
fn module_relative(path: &str, root: &str) -> String {
    let mut rel = graph::relative_path(path, root);
    for prefix in ["src/lua/", "src/", "lua/"] {
        if let Some(stripped) = rel.strip_prefix(prefix) {
            rel = stripped.to_owned();
        }
    }
    let rel = rel.strip_suffix(".lua").unwrap_or(&rel);
    let rel = if rel == "init" {
        ""
    } else {
        rel.strip_suffix("/init").unwrap_or(rel)
    };
    rel.replace('/', ".")
}

pub struct LuaScanner;

pub static SCANNER: LuaScanner = LuaScanner;

impl Scanner for LuaScanner {
    fn accepts(&self, relative: &str, opts: &Value) -> bool {
        if !relative.ends_with(".lua") || relative.ends_with("_spec.lua") {
            return false;
        }
        let excluded: Vec<&str> = seq(get(opts, "exclude"))
            .iter()
            .filter_map(|v| match v {
                Value::String(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        let mut rest = relative;
        while let Some((dir, tail)) = rest.split_once('/') {
            if dir.starts_with('.') || SKIP_DIRS.contains(&dir) || excluded.contains(&dir) {
                return false;
            }
            rest = tail;
        }
        true
    }

    fn accepts_directory(&self, relative: &str, opts: &Value) -> bool {
        self.accepts(&format!("{relative}/__module.lua"), opts)
    }

    fn scan(
        &self,
        root: &str,
        opts: &Value,
        files: &[crate::files::SourceFile],
        cwd: &str,
    ) -> Result<Value, String> {
        let prefix = get(opts, "prefix")
            .cloned()
            .unwrap_or(Value::String("app".into()));
        let prefix = match &prefix {
            Value::String(s) => s.clone(),
            _ => String::new(),
        };
        let ns_prefix = get(opts, "ns-prefix")
            .and_then(|v| match v {
                Value::String(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_else(|| prefix.clone());
        let mut sorted: Vec<&crate::files::SourceFile> = files
            .iter()
            .filter(|f| self.accepts(&graph::relative_path(&f.path, root), opts))
            .collect();
        sorted.sort_by(|a, b| a.path.cmp(&b.path));
        let mut metas = Vec::new();
        let mut by_rel: Vec<(String, Value)> = Vec::new();
        let mut by_ns: Vec<(String, Value)> = Vec::new();
        for f in &sorted {
            let rel = module_relative(&f.path, root);
            let ns = if rel == ns_prefix || rel.starts_with(&format!("{ns_prefix}.")) {
                rel.clone()
            } else {
                graph::ns_join(&ns_prefix, &rel)
            };
            let cid = graph::id_of(&ns, Some(&Value::String(prefix.clone())));
            metas.push((f, rel.clone(), ns.clone(), cid.clone()));
            if !rel.is_empty() {
                by_rel.push((rel, cid.clone()));
            }
            if !ns.is_empty() {
                by_ns.push((ns, cid));
            }
        }
        let mut classes = Vec::new();
        let mut edges: Vec<Value> = Vec::new();
        let mut foreigns: Vec<Value> = Vec::new();
        for (f, _, ns, cid) in &metas {
            let surface = read_module(&f.source);
            let mut ops = Vec::new();
            for d in seq(get(&surface, "defns")) {
                if !flag(d, "local") && !flag(d, "nested") {
                    ops.push(record(vec![
                        ("name", get(d, "name").cloned().unwrap_or(Value::Nil)),
                        ("text", get(d, "name").cloned().unwrap_or(Value::Nil)),
                    ]));
                }
            }
            classes.push(record(vec![
                ("id", cid.clone()),
                ("ns", Value::String(ns.clone())),
                ("name", Value::String(graph::module_name(cid))),
                ("lang", Value::keyword("lua")),
                ("file", Value::String(graph::relative_path(&f.path, cwd))),
                ("ops", Value::Vector(ops)),
            ]));
            let edge = |edges: &mut Vec<Value>, to: Value, kind: &str| {
                if *cid != to {
                    let e = record(vec![
                        ("from", cid.clone()),
                        ("to", to),
                        ("kind", Value::keyword(kind)),
                    ]);
                    if !edges.contains(&e) {
                        edges.push(e);
                    }
                }
            };
            for n in seq(get(&surface, "bases")) {
                let name = match n {
                    Value::String(s) => s.clone(),
                    _ => continue,
                };
                let target = by_rel
                    .iter()
                    .find(|(r, _)| *r == name)
                    .or_else(|| by_ns.iter().find(|(r, _)| *r == name))
                    .map(|(_, cid)| cid.clone());
                if let Some(target) = target {
                    edge(&mut edges, target, "inheritance");
                }
            }
            for n in seq(get(&surface, "requires")) {
                let name = match n {
                    Value::String(s) => s.clone(),
                    _ => continue,
                };
                let target = by_rel
                    .iter()
                    .find(|(r, _)| *r == name)
                    .or_else(|| by_ns.iter().find(|(r, _)| *r == name))
                    .map(|(_, cid)| cid.clone());
                let target = match target {
                    Some(target) => target,
                    None => {
                        let foreign = Value::keyword(&name);
                        if !foreigns.contains(&foreign) {
                            foreigns.push(foreign.clone());
                        }
                        foreign
                    }
                };
                edge(&mut edges, target, "dependency");
            }
        }
        for cid in foreigns {
            classes.push(graph::foreign_class(&cid));
        }
        Ok(record(vec![
            ("classes", Value::Vector(classes)),
            ("edges", Value::Vector(edges)),
        ]))
    }
}
