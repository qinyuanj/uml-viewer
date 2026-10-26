//! Port of `love/uml/lua_language/graph_lua.lua`: a pure Lua module scanner
//! over caller-supplied canonical source snapshots. Scanner limitations are
//! preserved (the goldens encode them); positions are 1-based like Lua.

use edn::Value;

use crate::scan::{self, Scanner, SourceFile};
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
            finish = Some(text::scan_quoted(source, i, c, None));
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

/// A `require` call at `i`: a literal module name, or a call whose argument
/// is an expression (`require(name)`) and so cannot be resolved statically.
enum Require {
    Module(Vec<u8>),
    Dynamic,
}

fn require_at(text: &[u8], i: usize) -> Option<Require> {
    if !keyword(text, i, b"require") {
        return None;
    }
    let after = space_tab_skip(text::sub(text, (i + 7) as i64, text.len() as i64));
    let mut tail = rest_of_line(after);
    let call = tail.first() == Some(&b'(');
    if call {
        tail = rest_of_line(space_tab_skip(&tail[1..]));
    }
    match tail.first() {
        Some(b'"' | b'\'') => {
            let s = &tail[1..];
            if !s.first().is_some_and(|&c| is_alpha_(c)) {
                return None;
            }
            let len = s.iter().take_while(|&&c| is_word(c) || c == b'.').count();
            Some(Require::Module(s[..len].to_vec()))
        }
        Some(&c) if call && is_alpha_(c) && !matches!(at(text, i - 1), Some(b'.' | b':')) => {
            Some(Require::Dynamic)
        }
        _ => None,
    }
}

fn skip_ws(t: &[u8], p: usize) -> usize {
    p + t[p.min(t.len())..]
        .iter()
        .take_while(|&&c| is_space(c))
        .count()
}

/// An identifier at 0-based `p`: its end, or `None`.
fn ident_end(t: &[u8], p: usize) -> Option<usize> {
    if !t.get(p).is_some_and(|&c| is_alpha_(c)) {
        return None;
    }
    Some(p + t[p..].iter().take_while(|&&c| is_word(c)).count())
}

/// `{ "a.b", 'c' }` at 0-based `p`: the module names of a constructor that
/// holds only string literals; anything else is not a literal list.
fn literal_modules(t: &[u8], p: usize) -> Option<Vec<Vec<u8>>> {
    if t.get(p) != Some(&b'{') {
        return None;
    }
    let mut out = Vec::new();
    let mut p = skip_ws(t, p + 1);
    while t.get(p) != Some(&b'}') {
        let quote = *t.get(p).filter(|&&c| c == b'"' || c == b'\'')?;
        let start = p + 1;
        let len = t[start..]
            .iter()
            .take_while(|&&c| c != quote && c != b'\n')
            .count();
        let name = &t[start..start + len];
        let module = name.first().is_some_and(|&c| is_alpha_(c))
            && name.iter().all(|&c| is_word(c) || c == b'.');
        if !module || t.get(start + len) != Some(&quote) {
            return None;
        }
        out.push(name.to_vec());
        p = skip_ws(t, start + len + 1);
        if matches!(t.get(p), Some(b',' | b';')) {
            p = skip_ws(t, p + 1);
        }
    }
    (!out.is_empty()).then_some(out)
}

/// The literal list bound by the last `local name = {` before 0-based `end`.
fn local_literal_modules(t: &[u8], name: &[u8], end: usize) -> Option<Vec<Vec<u8>>> {
    (0..end).rev().find_map(|j| {
        if !keyword(t, j + 1, b"local") {
            return None;
        }
        let p = skip_ws(t, j + 5);
        let e = ident_end(t, p)?;
        if &t[p..e] != name {
            return None;
        }
        let p = skip_ws(t, e);
        (t.get(p) == Some(&b'=')).then(|| literal_modules(t, skip_ws(t, p + 1)))?
    })
}

/// Whether the block opened just before 0-based `from` is still open at `to`.
fn block_open(t: &[u8], holes: &[(usize, usize)], from: usize, to: usize) -> bool {
    let mut depth = 1usize;
    for j in from + 1..=to {
        if in_hole(holes, j) {
            continue;
        }
        if ["function", "if", "do", "repeat"]
            .iter()
            .any(|w| keyword(t, j, w.as_bytes()))
        {
            depth += 1;
        } else if keyword(t, j, b"end") || keyword(t, j, b"until") {
            depth -= 1;
            if depth == 0 {
                return false;
            }
        }
    }
    true
}

/// `require(name)` at 1-based `i` where `name` is the value of the nearest
/// open `for _, name in ipairs(list) do` over a literal list (inline, or a
/// `local list = {...}`): the listed modules, which are static after all.
fn loop_literal_modules(t: &[u8], holes: &[(usize, usize)], i: usize) -> Option<Vec<Vec<u8>>> {
    let p = skip_ws(t, i - 1 + 7);
    let p = skip_ws(t, p + (t.get(p) == Some(&b'(')) as usize);
    let name_end = ident_end(t, p)?;
    let name = &t[p..name_end];
    if t.get(skip_ws(t, name_end)) != Some(&b')') {
        return None;
    }
    let f = (0..i - 1)
        .rev()
        .find(|&j| !in_hole(holes, j + 1) && keyword(t, j + 1, b"for"))?;
    let q = skip_ws(t, f + 3);
    let q = skip_ws(t, ident_end(t, q)?);
    if t.get(q) != Some(&b',') {
        return None;
    }
    let q = skip_ws(t, q + 1);
    let e = ident_end(t, q)?;
    if &t[q..e] != name {
        return None;
    }
    let q = skip_ws(t, e);
    if !keyword(t, q + 1, b"in") {
        return None;
    }
    let q = skip_ws(t, q + 2);
    let e = ident_end(t, q)?;
    if !matches!(&t[q..e], b"ipairs" | b"pairs") {
        return None;
    }
    let q = skip_ws(t, e);
    if t.get(q) != Some(&b'(') {
        return None;
    }
    let q = skip_ws(t, q + 1);
    let (modules, q) = match ident_end(t, q) {
        Some(e) => (local_literal_modules(t, &t[q..e], f)?, e),
        None => {
            let close = q + t[q..].iter().position(|&c| c == b'}')?;
            (literal_modules(t, q)?, close + 1)
        }
    };
    let q = skip_ws(t, q);
    if t.get(q) != Some(&b')') {
        return None;
    }
    let q = skip_ws(t, q + 1);
    (keyword(t, q + 1, b"do") && block_open(t, holes, q + 2, i - 1)).then_some(modules)
}

/// 1-based line of position `i`.
fn line_of(text: &[u8], i: usize) -> i64 {
    text[..i - 1].iter().filter(|&&c| c == b'\n').count() as i64 + 1
}

/// Table-constructor depth after the byte `c`.
fn track_brace(braces: usize, c: Option<u8>) -> usize {
    match c {
        Some(b'{') => braces + 1,
        Some(b'}') => braces.saturating_sub(1),
        _ => braces,
    }
}

/// Opens or closes a block for the keyword; `true` marks a function body.
fn track_block(blocks: &mut Vec<bool>, kw: Option<&str>) {
    match kw {
        Some("function") => blocks.push(true),
        Some("if" | "do" | "repeat") => blocks.push(false),
        Some("end" | "until") => {
            blocks.pop();
        }
        _ => {}
    }
}

/// The stderr line for a file's unresolved `require(name)` calls, if any.
fn dynamic_require_warning(file: &str, lines: &[Value]) -> Option<String> {
    let at: Vec<String> = lines
        .iter()
        .filter_map(|l| match l {
            Value::Integer(n) => Some(format!("{file}:{n}")),
            _ => None,
        })
        .collect();
    (!at.is_empty()).then(|| format!("Unresolved dynamic require: {}", at.join(", ")))
}

/// `read_module(source)`: requires, definitions and setmetatable bases.
/// A definition is nested inside a function body or a table constructor,
/// as crapper counts entries; a top-level `if`/`do` block does not nest.
/// `require(name)` cannot be resolved statically; its line is reported,
/// unless `name` iterates a literal module list.
pub fn read_module(source: &[u8]) -> Value {
    let (text, holes) = mask(source);
    let mut requires = Vec::new();
    let mut defns = Vec::new();
    let mut bases: Vec<Value> = Vec::new();
    let mut bindings: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
    let mut base_names: Vec<Vec<u8>> = Vec::new();
    let mut dynamic_requires = Vec::new();
    // Open blocks, innermost last: `true` for a function body.
    let mut blocks: Vec<bool> = Vec::new();
    let mut braces: usize = 0;
    for i in 1..=text.len() {
        if in_hole(&holes, i) {
            continue;
        }
        match require_at(&text, i) {
            Some(Require::Module(m)) => requires.push(Value::String(text::text(&m))),
            Some(Require::Dynamic) => match loop_literal_modules(&text, &holes, i) {
                Some(modules) => {
                    requires.extend(modules.iter().map(|m| Value::String(text::text(m))))
                }
                None => dynamic_requires.push(Value::Integer(line_of(&text, i))),
            },
            None => {}
        }
        let c = at(&text, i);
        braces = track_brace(braces, c);
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
                        ("nested", Value::Bool(braces > 0 || blocks.contains(&true))),
                    ]));
                }
            }
            if blocks.is_empty() && keyword(&text, i, b"setmetatable") {
                let rest = text::sub(&text, i as i64, text.len() as i64);
                if let Some(tail) = match_setmetatable(rest)
                    && let Some(base) = match_base(&tail)
                {
                    base_names.push(base);
                }
            }
            track_block(&mut blocks, kw_found);
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
        ("dynamic-requires", Value::Vector(dynamic_requires)),
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
    let mut rel = scan::relative_path(path, root);
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
        files: &[SourceFile],
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
        let mut sorted: Vec<&SourceFile> = files
            .iter()
            .filter(|f| self.accepts(&scan::relative_path(&f.path, root), opts))
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
                scan::ns_join(&ns_prefix, &rel)
            };
            let cid = scan::id_of(&ns, Some(&Value::String(prefix.clone())));
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
            let file = scan::relative_path(&f.path, cwd);
            if let Some(warning) =
                dynamic_require_warning(&file, seq(get(&surface, "dynamic-requires")))
            {
                eprintln!("{warning}");
            }
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
                ("name", Value::String(scan::module_name(cid))),
                ("lang", Value::keyword("lua")),
                ("file", Value::String(scan::relative_path(&f.path, cwd))),
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
            classes.push(scan::foreign_class(&cid));
        }
        Ok(record(vec![
            ("classes", Value::Vector(classes)),
            ("edges", Value::Vector(edges)),
        ]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The names `scan` turns into `:ops`: neither local nor nested.
    fn ops(source: &str) -> Vec<String> {
        let surface = read_module(source.as_bytes());
        seq(get(&surface, "defns"))
            .iter()
            .filter(|d| !flag(d, "local") && !flag(d, "nested"))
            .filter_map(|d| match get(d, "name") {
                Some(Value::String(n)) => Some(n.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn functions_in_a_table_constructor_are_not_operations() {
        let source = "local M = {}\nM.t = { f = function() end }\nfunction M.a() end\nreturn M\n";
        assert_eq!(vec!["M.a"], ops(source));
    }

    #[test]
    fn functions_in_top_level_blocks_are_operations_like_crapper() {
        let source = "local M = {}\n\
                      if jit then\n  function M.fast() end\nelse\n  M.slow = function() end\nend\n\
                      do function M.d() end end\n\
                      function M.outer()\n  function M.inner() end\nend\n\
                      return M\n";
        assert_eq!(vec!["M.fast", "M.slow", "M.d", "M.outer"], ops(source));
    }

    #[test]
    fn a_warning_names_each_dynamic_require_by_file_and_line() {
        assert_eq!(None, dynamic_require_warning("a.lua", &[]));
        assert_eq!(
            Some("Unresolved dynamic require: a.lua:3, a.lua:9".to_owned()),
            dynamic_require_warning("a.lua", &[Value::Integer(3), Value::Integer(9)])
        );
    }

    fn strings(values: &[Value]) -> Vec<&str> {
        values
            .iter()
            .filter_map(|v| match v {
                Value::String(s) => Some(s.as_str()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn a_require_over_a_literal_module_list_resolves_each_module() {
        let source = "local function load()\n\
                      \x20 local mods = {\n    \"a.b\",\n    'c', -- note\n  }\n\
                      \x20 for _, m in ipairs(mods) do\n    reg(require(m))\n  end\n\
                      \x20 for _, n in pairs({ \"d\"; \"e.f\" }) do require(n) end\n\
                      end\n";
        let surface = read_module(source.as_bytes());
        assert_eq!(
            vec!["a.b", "c", "d", "e.f"],
            strings(seq(get(&surface, "requires")))
        );
        assert!(seq(get(&surface, "dynamic-requires")).is_empty());
    }

    #[test]
    fn a_require_outside_a_literal_list_loop_stays_dynamic() {
        let source = "local mods = { \"a\", x }\n\
                      for _, m in ipairs(mods) do require(m) end\n\
                      local ok = { \"b\" }\n\
                      for _, m in ipairs(ok) do end\n\
                      require(m)\n\
                      for i, m in ipairs(ok) do require(i) end\n\
                      for _, m in ipairs(other) do require(m) end\n";
        let surface = read_module(source.as_bytes());
        assert!(seq(get(&surface, "requires")).is_empty());
        assert_eq!(
            &[
                Value::Integer(2),
                Value::Integer(5),
                Value::Integer(6),
                Value::Integer(7)
            ][..],
            seq(get(&surface, "dynamic-requires"))
        );
    }

    #[test]
    fn dynamic_requires_are_reported_with_their_lines() {
        let source = "local a = require \"x.y\"\n\
                      local name = 'z'\n\
                      local b = require(name)\n\
                      local c = pkg.require(other)\n\
                      -- require(nope)\n\
                      local d = require([[long]])\n";
        let surface = read_module(source.as_bytes());
        assert_eq!(
            &[Value::String("x.y".into())][..],
            seq(get(&surface, "requires"))
        );
        assert_eq!(
            &[Value::Integer(3)][..],
            seq(get(&surface, "dynamic-requires"))
        );
    }
}
