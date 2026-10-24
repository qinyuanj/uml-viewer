//! Port of `love/uml/python_language/graph_python.lua`: the JVM Python
//! scanner semantics over caller-owned canonical source snapshots.

use edn::Value;

use crate::graph::{self, Scanner};
use crate::text::{self, at, is_alpha_, is_space, is_word};
use crate::value::get_truthy as get;
use edn::record::{record, seq};

fn ident(c: Option<u8>) -> bool {
    c.is_some_and(is_word)
}

fn in_hole(holes: &[(usize, usize)], i: usize) -> bool {
    holes.iter().any(|&(a, b)| i >= a && i < b)
}

fn trim(s: &[u8]) -> &[u8] {
    text::trim(s)
}

/// `flags(s, i)`: string-prefix flags for a quote at 1-based `i`;
/// `(raw, f)` or `None` when `i` is not a string start.
fn flags(s: &[u8], i: usize) -> Option<(bool, bool)> {
    let q = at(s, i)?;
    if q != b'"' && q != b'\'' {
        return None;
    }
    let mut raw = false;
    let mut f = false;
    let mut j = i as i64 - 1;
    while j > 0
        && matches!(at(s, j as usize), Some(c) if matches!(c, b'r' | b'R' | b'u' | b'U' | b'b' | b'B' | b'f' | b'F'))
    {
        let c = at(s, j as usize).unwrap().to_ascii_lowercase();
        raw = raw || c == b'r';
        f = f || c == b'f';
        j -= 1;
    }
    if j == 0 || !ident(at(s, j as usize)) {
        return Some((raw, f));
    }
    None
}

/// `skip_string(s, i)`: one past the closing quote(s), or `#s + 1`.
fn skip_string(s: &[u8], i: usize) -> Option<usize> {
    let (raw, f) = flags(s, i)?;
    let q = at(s, i).unwrap();
    let len = if at(s, i + 1) == Some(q) && at(s, i + 2) == Some(q) {
        3
    } else {
        1
    };
    let mut j = i + len;
    let mut depth = 0i64;
    while j <= s.len() {
        let c = at(s, j).unwrap();
        let n = at(s, j + 1);
        if c == b'\\' && j < s.len() && (!raw || n == Some(q)) {
            j += 2;
        } else if f && (c == b'{' || c == b'}') {
            if n == Some(c) {
                // `{{`/`}}` are literal braces.
                j += 2;
            } else if c == b'{' {
                depth += 1;
                j += 1;
            } else if depth > 0 {
                depth -= 1;
                j += 1;
            } else {
                j += 1;
            }
        } else if depth > 0 && (c == b'"' || c == b'\'') {
            j = skip_string(s, j).unwrap_or(j + 1);
        } else if depth == 0 && (1..=len).all(|k| at(s, j + k - 1) == Some(q)) {
            return Some(j + len);
        } else {
            j += 1;
        }
    }
    Some(s.len() + 1)
}

/// `mask(s)`: `#` comments blanked; strings kept with holes.
fn mask(s: &[u8]) -> (Vec<u8>, Vec<(usize, usize)>) {
    let mut out = Vec::new();
    let mut holes = Vec::new();
    let mut i = 1;
    while i <= s.len() {
        if at(s, i) == Some(b'#') {
            let j = text::find_plain(s, b"\n", i).unwrap_or(s.len() + 1);
            out.extend(std::iter::repeat_n(b' ', j - i));
            i = j;
        } else if let Some(j) = skip_string(s, i) {
            out.extend_from_slice(&s[i - 1..j - 1]);
            holes.push((i, j));
            i = j;
        } else {
            out.push(at(s, i).unwrap());
            i += 1;
        }
    }
    (out, holes)
}

/// `statement_end(s, i)`: position of the statement-ending newline
/// (`#s + 1` at end of input).
fn statement_end(s: &[u8], mut i: usize) -> usize {
    let (mut p, mut b, mut c) = (0i64, 0i64, 0i64);
    while i <= s.len() {
        let x = at(s, i).unwrap();
        if x == b'\\' && at(s, i + 1) == Some(b'\n') {
            i += 2;
        } else if x == b'\n' && p == 0 && b == 0 && c == 0 {
            return i;
        } else {
            match x {
                b'(' => p += 1,
                b')' => p = (p - 1).max(0),
                b'[' => b += 1,
                b']' => b = (b - 1).max(0),
                b'{' => c += 1,
                b'}' => c = (c - 1).max(0),
                _ => {}
            }
            i += 1;
        }
    }
    s.len() + 1
}

/// `^([%a_][%w_%.]*)%s+as%s+([%a_][%w_]*)$` or `^([%a_][%w_%.]*)$`.
fn parse_part(part: &[u8]) -> Option<(Vec<u8>, Option<Vec<u8>>)> {
    let dotted = |s: &[u8]| -> Option<usize> {
        if !s.first().is_some_and(|&c| is_alpha_(c)) {
            return None;
        }
        Some(s.iter().take_while(|&&c| is_word(c) || c == b'.').count())
    };
    let plain = |s: &[u8]| -> Option<usize> {
        if !s.first().is_some_and(|&c| is_alpha_(c)) {
            return None;
        }
        Some(s.iter().take_while(|&&c| is_word(c)).count())
    };
    if let Some(len) = dotted(part) {
        let rest = &part[len..];
        let spaces = rest.iter().take_while(|&&c| is_space(c)).count();
        if spaces > 0
            && let Some(after) = rest[spaces..].strip_prefix(b"as")
        {
            let spaces2 = after.iter().take_while(|&&c| is_space(c)).count();
            if spaces2 > 0 {
                let rest2 = &after[spaces2..];
                if let Some(blen) = plain(rest2)
                    && blen == rest2.len()
                {
                    return Some((part[..len].to_vec(), Some(rest2[..blen].to_vec())));
                }
            }
        }
        if len == part.len() {
            return Some((part[..len].to_vec(), None));
        }
    }
    None
}

/// `parts(s)`: comma-split import names with `as` bindings.
fn parts(s: &[u8]) -> Vec<Value> {
    let mut out = Vec::new();
    let cleaned: Vec<u8> = s
        .iter()
        .map(|&c| if c == b'(' || c == b')' { b' ' } else { c })
        .collect();
    for part in cleaned.split(|&c| c == b',') {
        let part = trim(part);
        if part.is_empty() {
            continue;
        }
        if let Some((name, bind)) = parse_part(part) {
            let bind = bind.unwrap_or_else(|| {
                let first = name.iter().take_while(|&&c| c != b'.').count();
                name[..first].to_vec()
            });
            out.push(record(vec![
                ("name", Value::String(text::text(&name))),
                ("bind", Value::String(text::text(&bind))),
            ]));
        }
    }
    out
}

fn strip_spaces(s: &[u8]) -> &[u8] {
    &s[s.iter().take_while(|&&c| is_space(c)).count()..]
}

/// `parse_import(s)`: the statement text without its trailing newline.
fn parse_import(s: &[u8]) -> Option<Value> {
    // trim(s:gsub('\\\n',' '):gsub('%s+',' '))
    let mut collapsed = Vec::with_capacity(s.len());
    let mut i = 0;
    while i < s.len() {
        if s[i] == b'\\' && s.get(i + 1) == Some(&b'\n') {
            collapsed.push(b' ');
            i += 2;
        } else if is_space(s[i]) {
            collapsed.push(b' ');
            i += 1;
            while i < s.len() && is_space(s[i]) {
                i += 1;
            }
        } else {
            collapsed.push(s[i]);
            i += 1;
        }
    }
    let s = trim(&collapsed);
    if s.starts_with(b"from ") {
        // `^from%s+([^ ]*)%s+import%s+(.+)$` — whitespace already collapsed.
        let rest = &s[5..];
        let path_end = rest.iter().position(|&c| c == b' ')?;
        let path = &rest[..path_end];
        let rest = strip_spaces(&rest[path_end..]);
        let rest = rest.strip_prefix(b"import")?;
        let names = strip_spaces(rest);
        if names.is_empty() {
            return None;
        }
        let dots = path.iter().take_while(|&&c| c == b'.').count();
        let module = &path[dots..];
        let valid = module.is_empty()
            || (module.first().is_some_and(|&c| is_alpha_(c))
                && module.iter().all(|&c| is_word(c) || c == b'.'));
        if valid {
            return Some(record(vec![
                ("kind", Value::keyword("from")),
                ("dots", Value::Integer(dots as i64)),
                (
                    "module",
                    if module.is_empty() {
                        Value::Nil
                    } else {
                        Value::String(text::text(module))
                    },
                ),
                ("names", Value::Vector(parts(names))),
            ]));
        }
        return None;
    }
    if s.starts_with(b"import ") {
        return Some(record(vec![
            ("kind", Value::keyword("import")),
            ("modules", Value::Vector(parts(&s[7..]))),
        ]));
    }
    None
}

/// `read_module(source)`: imports and top-level defs.
pub fn read_module(source: &[u8]) -> Value {
    let (text, holes) = mask(source);
    let mut imports = Vec::new();
    let mut defs = Vec::new();
    let mut i = 1;
    while i <= text.len() {
        // `text:sub(i):match('^(%a+)')`
        let word_len = text[i - 1..]
            .iter()
            .take_while(|&&c| c.is_ascii_alphabetic())
            .count();
        let word = &text[i - 1..i - 1 + word_len];
        if (word == b"from" || word == b"import")
            && !ident(at(&text, i.wrapping_sub(1)))
            && !ident(at(&text, i + word_len))
            && !in_hole(&holes, i)
            && statement_is_clean(&text, i)
        {
            let finish = statement_end(&text, i);
            if let Some(imp) = parse_import(&text[i - 1..finish - 1]) {
                imports.push(imp);
            }
            i = finish;
        } else {
            i += 1;
        }
    }
    let mut i = 1;
    while i <= text.len() {
        let line_len = text[i - 1..]
            .iter()
            .position(|&c| c == b'\n')
            .unwrap_or(text.len() - i + 1);
        let line = &text[i - 1..i - 1 + line_len];
        let name = match_def_name(line, b"async", true)
            .or_else(|| match_def_name(line, b"def", false))
            .or_else(|| match_def_name(line, b"class", false));
        if let Some(name) = name
            && name.first() != Some(&b'_')
            && !in_hole(&holes, i)
        {
            // `header:gsub('%s+',' ')` — collapse each whitespace run.
            let end = statement_end(&text, i);
            let raw = &text[i - 1..end - 1];
            let mut header = Vec::with_capacity(raw.len());
            let mut in_space = false;
            for &c in raw {
                if is_space(c) {
                    if !in_space {
                        header.push(b' ');
                    }
                    in_space = true;
                } else {
                    header.push(c);
                    in_space = false;
                }
            }
            if line.starts_with(b"class") {
                let body = match_class_bases(&header);
                let mut bases = Vec::new();
                if let Some(body) = &body {
                    for part in body.split(|&c| c == b',') {
                        let part = trim(part);
                        if part.is_empty() {
                            continue;
                        }
                        let valid = part.first().is_some_and(|&c| is_alpha_(c))
                            && part.iter().all(|&c| is_word(c) || c == b'.');
                        if valid {
                            bases.push(Value::String(text::text(part)));
                        }
                    }
                }
                defs.push(record(vec![
                    ("name", Value::String(text::text(&name))),
                    ("kind", Value::keyword("class")),
                    (
                        "bases",
                        body.map(|_| Value::Vector(bases)).unwrap_or(Value::Nil),
                    ),
                ]));
            } else {
                defs.push(record(vec![
                    ("name", Value::String(text::text(&name))),
                    ("kind", Value::keyword("function")),
                    (
                        "return",
                        match_return(&header)
                            .map(Value::String)
                            .unwrap_or(Value::Nil),
                    ),
                ]));
            }
        }
        i += line_len + 1;
    }
    record(vec![
        ("imports", Value::Vector(imports)),
        ("defs", Value::Vector(defs)),
    ])
}

/// The statement-before check: `([^\n;]*)$` then `^%s*$`.
fn statement_is_clean(text: &[u8], i: usize) -> bool {
    let before = &text[..i - 1];
    let start = before
        .iter()
        .rposition(|&c| c == b'\n' || c == b';')
        .map_or(0, |p| p + 1);
    before[start..].iter().all(|&c| is_space(c))
}

/// `^async[ \t]+def[ \t]+(name)` (async_def) or `^def`/`^class` + name.
fn match_def_name(line: &[u8], word: &[u8], async_def: bool) -> Option<Vec<u8>> {
    let rest = line.strip_prefix(word)?;
    let spaces = rest
        .iter()
        .take_while(|&&c| c == b' ' || c == b'\t')
        .count();
    if spaces == 0 {
        return None;
    }
    let rest = &rest[spaces..];
    let rest = if async_def {
        let rest = rest.strip_prefix(b"def")?;
        let spaces = rest
            .iter()
            .take_while(|&&c| c == b' ' || c == b'\t')
            .count();
        if spaces == 0 {
            return None;
        }
        &rest[spaces..]
    } else {
        rest
    };
    if !rest.first().is_some_and(|&c| is_alpha_(c)) {
        return None;
    }
    let len = rest.iter().take_while(|&&c| is_word(c)).count();
    Some(rest[..len].to_vec())
}

/// `%(([^)]*)%)%s*:` on the collapsed header.
fn match_class_bases(header: &[u8]) -> Option<Vec<u8>> {
    let open = header.iter().position(|&c| c == b'(')?;
    let close = header[open + 1..]
        .iter()
        .position(|&c| c == b')')
        .map(|p| open + 1 + p)?;
    let rest = strip_spaces(&header[close + 1..]);
    if rest.first() == Some(&b':') {
        Some(header[open + 1..close].to_vec())
    } else {
        None
    }
}

/// `%)%s*%->%s*([%a_][%w_]*)%s*:` on the collapsed header.
fn match_return(header: &[u8]) -> Option<String> {
    let mut search = 0;
    while let Some(off) = header[search.min(header.len())..]
        .iter()
        .position(|&c| c == b')')
    {
        let p = search + off;
        let rest = strip_spaces(&header[p + 1..]);
        if let Some(rest) = rest.strip_prefix(b"->") {
            let rest = strip_spaces(rest);
            if rest.first().is_some_and(|&c| is_alpha_(c)) {
                let len = rest.iter().take_while(|&&c| is_word(c)).count();
                let after = strip_spaces(&rest[len..]);
                if after.first() == Some(&b':') {
                    return Some(text::text(&rest[..len]));
                }
            }
        }
        search = p + 1;
    }
    None
}

const SKIP_DIRS: &[&str] = &[
    "__pycache__",
    "venv",
    "site-packages",
    "dist",
    "build",
    "node_modules",
    "tests",
    "__pypackages__",
];

/// `accepts_directory(relative)`.
fn accepts_directory(relative: &str) -> bool {
    for dir in relative.split('/') {
        if dir.is_empty() {
            continue;
        }
        if dir.starts_with('.') || SKIP_DIRS.contains(&dir) {
            return false;
        }
    }
    true
}

/// `accepts(relative)`.
fn accepts(relative: &str) -> bool {
    let name = relative.rsplit('/').next().unwrap_or("");
    if !name.ends_with(".py")
        || name == "conftest.py"
        || is_test_file(name, "test_", ".py")
        || is_test_file(name, "", "_test.py")
        || is_test_file(name, "", "_tests.py")
    {
        return false;
    }
    let dir = relative.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
    accepts_directory(dir)
}

/// `^test_.+%.py$` / `^.+_test%.py$` / `^.+_tests%.py$` shape checks.
fn is_test_file(name: &str, prefix: &str, suffix: &str) -> bool {
    name.len() > prefix.len() + suffix.len() && name.starts_with(prefix) && name.ends_with(suffix)
}

fn parent(s: &str) -> Option<&str> {
    s.rsplit_once('.').map(|(p, _)| p)
}

fn join(a: &str, b: &str) -> String {
    if b.is_empty() {
        return a.to_owned();
    }
    if a.is_empty() {
        return b.to_owned();
    }
    format!("{a}.{b}")
}

/// `marker(s)`: Protocol/ABC/ABCMeta as the last dotted segment.
fn marker(s: &str) -> bool {
    let n = s.rsplit('.').next().unwrap_or(s);
    n == "Protocol" || n == "ABC" || n == "ABCMeta"
}

pub struct PythonScanner;

pub static SCANNER: PythonScanner = PythonScanner;

/// A resolved import hit.
enum Hit {
    Id {
        id: Value,
        bind: Option<String>,
        path: Option<String>,
    },
    Foreign(String),
}

impl Scanner for PythonScanner {
    fn accepts(&self, relative: &str, _opts: &Value) -> bool {
        accepts(relative)
    }

    fn accepts_directory(&self, relative: &str, _opts: &Value) -> bool {
        accepts_directory(relative)
    }

    fn scan(
        &self,
        root: &str,
        opts: &Value,
        files: &[crate::files::SourceFile],
        cwd: &str,
    ) -> Result<Value, String> {
        let prefix = match get(opts, "prefix") {
            Some(Value::String(s)) => s.clone(),
            _ => "app".to_owned(),
        };
        let ns_prefix = match get(opts, "ns-prefix") {
            Some(Value::String(s)) => s.clone(),
            _ => prefix.clone(),
        };
        let mut sorted: Vec<&crate::files::SourceFile> = files
            .iter()
            .filter(|f| accepts(&graph::relative_path(&f.path, root)))
            .collect();
        sorted.sort_by(|a, b| a.path.cmp(&b.path));
        struct Meta {
            file_path: String,
            source: Vec<u8>,
            ns: String,
            id: Value,
            init: bool,
        }
        let mut metas = Vec::new();
        let mut by_ns: Vec<(String, Value)> = Vec::new();
        let mut by_rel: Vec<(String, Value)> = Vec::new();
        for f in &sorted {
            let mut rel = graph::relative_path(&f.path, root);
            rel = rel.strip_suffix(".py").unwrap_or(&rel).to_owned();
            if rel == "__init__" {
                rel = String::new();
            } else if let Some(stripped) = rel.strip_suffix("/__init__") {
                rel = stripped.to_owned();
            }
            let rel = rel.replace('/', ".");
            let ns = if rel == ns_prefix || rel.starts_with(&format!("{ns_prefix}.")) {
                rel.clone()
            } else {
                graph::ns_join(&ns_prefix, &rel)
            };
            let id = graph::id_of(&ns, Some(&Value::String(prefix.clone())));
            let init = f.path.ends_with("/__init__.py");
            by_ns.push((ns.clone(), id.clone()));
            if !rel.is_empty() {
                by_rel.push((rel, id.clone()));
            }
            metas.push(Meta {
                file_path: f.path.clone(),
                source: f.source.clone(),
                ns,
                id,
                init,
            });
        }
        let exact = |s: &str| -> Option<Value> {
            if s.is_empty() {
                return None;
            }
            by_ns
                .iter()
                .find(|(n, _)| n == s)
                .or_else(|| by_rel.iter().find(|(n, _)| n == s))
                .map(|(_, id)| id.clone())
        };
        let mut classes = Vec::new();
        let mut edges: Vec<Value> = Vec::new();
        let mut foreigns: Vec<Value> = Vec::new();
        let project: std::collections::HashSet<String> = metas
            .iter()
            .map(|m| match &m.id {
                Value::Keyword(n) => n.clone(),
                _ => String::new(),
            })
            .collect();
        for m in &metas {
            let surface = read_module(&m.source);
            let mut hits: Vec<Hit> = Vec::new();
            let package = if m.init {
                m.ns.clone()
            } else {
                parent(&m.ns).unwrap_or("").to_owned()
            };
            for imp in seq(get(&surface, "imports")) {
                if get(imp, "kind") == Some(&Value::keyword("import")) {
                    for part in seq(get(imp, "modules")) {
                        let n = match get(part, "name") {
                            Some(Value::String(s)) => s.clone(),
                            _ => continue,
                        };
                        let bind = match get(part, "bind") {
                            Some(Value::String(s)) => Some(s.clone()),
                            _ => None,
                        };
                        match exact(&n) {
                            Some(id) => hits.push(Hit::Id {
                                id,
                                bind,
                                path: Some(n),
                            }),
                            None => hits.push(Hit::Foreign(n)),
                        }
                    }
                } else {
                    let dots = match get(imp, "dots") {
                        Some(Value::Integer(d)) => *d as usize,
                        _ => 0,
                    };
                    let module = match get(imp, "module") {
                        Some(Value::String(s)) => Some(s.clone()),
                        _ => None,
                    };
                    let mut rootmod = module.clone();
                    if dots > 0 {
                        let mut rm = Some(package.clone());
                        let mut left = dots - 1;
                        while left > 0 {
                            match rm.as_deref() {
                                Some("") | None => {
                                    rm = None;
                                    break;
                                }
                                Some(current) => {
                                    rm = Some(parent(current).unwrap_or("").to_owned());
                                }
                            }
                            left -= 1;
                        }
                        rootmod = rm.map(|rm| join(&rm, module.as_deref().unwrap_or("")));
                    }
                    if let Some(rootmod) = rootmod {
                        let relative_only = dots > 0 && module.as_deref().is_none_or(str::is_empty);
                        let rootid = exact(&rootmod);
                        let mut found = false;
                        for part in seq(get(imp, "names")) {
                            let name = match get(part, "name") {
                                Some(Value::String(s)) => s.clone(),
                                _ => continue,
                            };
                            let bind = match get(part, "bind") {
                                Some(Value::String(s)) => Some(s.clone()),
                                _ => None,
                            };
                            let id = exact(&join(&rootmod, &name))
                                .or_else(|| if relative_only { None } else { rootid.clone() });
                            if let Some(id) = id {
                                hits.push(Hit::Id {
                                    id,
                                    bind,
                                    path: None,
                                });
                                found = true;
                            }
                        }
                        if !found && !relative_only {
                            if let Some(rootid) = rootid {
                                hits.push(Hit::Id {
                                    id: rootid,
                                    bind: None,
                                    path: None,
                                });
                            } else if !rootmod.is_empty() {
                                hits.push(Hit::Foreign(rootmod));
                            }
                        }
                    }
                }
            }
            let mut bindings: Vec<(String, Value)> = Vec::new();
            let mut paths: Vec<(String, Value)> = Vec::new();
            let mut requires: Vec<Value> = Vec::new();
            let mut external: Vec<Value> = Vec::new();
            for h in &hits {
                match h {
                    Hit::Id { id, bind, path } => {
                        if let Some(bind) = bind {
                            bindings.retain(|(k, _)| k != bind);
                            bindings.push((bind.clone(), id.clone()));
                        }
                        if let Some(path) = path {
                            paths.retain(|(k, _)| k != path);
                            paths.push((path.clone(), id.clone()));
                        }
                        if !requires.contains(id) {
                            requires.push(id.clone());
                        }
                    }
                    Hit::Foreign(name) => {
                        let foreign = Value::keyword(name);
                        if !external.contains(&foreign) {
                            external.push(foreign.clone());
                        }
                        if !foreigns.contains(&foreign) {
                            foreigns.push(foreign);
                        }
                    }
                }
            }
            let target = |expr: &str| -> Option<Value> {
                if let Some((_, id)) = bindings.iter().find(|(k, _)| k == expr) {
                    return Some(id.clone());
                }
                if let Some((_, id)) = paths.iter().find(|(k, _)| k == expr) {
                    return Some(id.clone());
                }
                let mut p = parent(expr);
                while let Some(current) = p {
                    if let Some((_, id)) = paths.iter().find(|(k, _)| k == current) {
                        return Some(id.clone());
                    }
                    if let Some((_, id)) = bindings.iter().find(|(k, _)| k == current) {
                        return Some(id.clone());
                    }
                    p = parent(current);
                }
                None
            };
            let mut inherits: Vec<Value> = Vec::new();
            let mut impls: Vec<Value> = Vec::new();
            let mut ops: Vec<Value> = Vec::new();
            let mut class_count = 0;
            let mut interface = true;
            for d in seq(get(&surface, "defs")) {
                let n = get(d, "name").cloned().unwrap_or(Value::Nil);
                ops.push(record(vec![("name", n.clone()), ("text", n)]));
                if get(d, "kind") == Some(&Value::keyword("function")) {
                    interface = false;
                } else {
                    class_count += 1;
                    let bases = seq(get(d, "bases"));
                    if bases.is_empty() {
                        interface = false;
                    }
                    for base in bases {
                        if let Value::String(base) = base
                            && !marker(base)
                        {
                            interface = false;
                        }
                    }
                }
                for base in seq(get(d, "bases")) {
                    if let Value::String(base) = base
                        && let Some(id) = target(base)
                    {
                        let group = if marker(base) {
                            &mut impls
                        } else {
                            &mut inherits
                        };
                        if !group.contains(&id) {
                            group.push(id);
                        }
                    }
                }
            }
            for d in seq(get(&surface, "defs")) {
                if let Some(Value::String(ret)) = get(d, "return")
                    && let Some((_, id)) = bindings.iter().find(|(k, _)| k == ret)
                    && !impls.contains(id)
                {
                    impls.push(id.clone());
                }
            }
            let mut c = record(vec![
                ("id", m.id.clone()),
                ("ns", Value::String(m.ns.clone())),
                ("name", Value::String(graph::module_name(&m.id))),
                ("lang", Value::keyword("python")),
                (
                    "file",
                    Value::String(graph::relative_path(&m.file_path, cwd)),
                ),
            ]);
            if !ops.is_empty() {
                edn::record::put(&mut c, "ops", Value::Vector(ops));
            }
            if class_count > 0 && interface {
                edn::record::put(&mut c, "stereotype", Value::keyword("interface"));
            }
            classes.push(c);
            for (group, kind) in [
                (&impls, "implements"),
                (&inherits, "inheritance"),
                (&requires, "dependency"),
                (&external, "dependency"),
            ] {
                for to in group {
                    if m.id != *to {
                        let e = record(vec![
                            ("from", m.id.clone()),
                            ("to", to.clone()),
                            ("kind", Value::keyword(kind)),
                        ]);
                        if !edges.contains(&e) {
                            edges.push(e);
                        }
                    }
                }
            }
        }
        for id in foreigns {
            let name = match &id {
                Value::Keyword(n) => n.clone(),
                _ => String::new(),
            };
            if !project.contains(&name) {
                classes.push(graph::foreign_class(&id));
            }
        }
        Ok(record(vec![
            ("classes", Value::Vector(classes)),
            ("edges", Value::Vector(edges)),
        ]))
    }
}
