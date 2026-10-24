//! Port of `love/uml/typescript_language/graph_typescript.lua`: a regex/string
//! scanner over caller-owned paths. Scanner limitations are preserved.

use std::collections::{HashMap, HashSet};

use edn::Value;

use crate::graph::{self, Scanner};
use crate::text::{self, at, is_space, is_ts_ident};
use crate::value::get_truthy as get;
use edn::record::{flag, record, seq};

fn ident(c: Option<u8>) -> bool {
    c.is_some_and(is_ts_ident)
}

fn is_id_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_' || c == b'$'
}

/// Captures `ID` = `[A-Za-z_$][%w_$]*` at the start of `s`.
fn capture_id(s: &[u8]) -> Option<usize> {
    if !s.first().is_some_and(|&c| is_id_start(c)) {
        return None;
    }
    Some(s.iter().take_while(|&&c| is_ts_ident(c)).count())
}

fn strip_spaces(s: &[u8]) -> &[u8] {
    &s[s.iter().take_while(|&&c| is_space(c)).count()..]
}

fn trim(s: &[u8]) -> &[u8] {
    text::trim(s)
}

/// `part:match('%f[%w_$]as%s+(ID)%s*$')`: a trailing `as name`.
fn match_as_name(part: &[u8]) -> Option<Vec<u8>> {
    // Frontier before 'as': walk candidate splits from the left.
    let mut search = 0;
    while let Some(off) = find_subslice(&part[search.min(part.len())..], b"as") {
        let p = search + off;
        let before_ok = p == 0 || !is_ts_ident(part[p - 1]);
        let after = &part[p + 2..];
        let spaces = after.iter().take_while(|&&c| is_space(c)).count();
        if before_ok && spaces > 0 {
            let rest = &after[spaces..];
            if let Some(len) = capture_id(rest)
                && trim(&rest[len..]).is_empty()
            {
                return Some(rest[..len].to_vec());
            }
        }
        search = p + 2;
    }
    None
}

/// `brace_names(stmt)`: names inside the first `{...}`, comma-separated.
fn brace_names(stmt: &[u8]) -> Vec<Value> {
    let mut names = Vec::new();
    let body: &[u8] = match stmt.iter().position(|&c| c == b'{') {
        Some(open) => {
            let rest = &stmt[open + 1..];
            let close = rest.iter().position(|&c| c == b'}').unwrap_or(rest.len());
            &rest[..close]
        }
        None => b"",
    };
    for part in body.split(|&c| c == b',') {
        let part = trim(part);
        if part.is_empty() {
            continue; // gmatch('[^,]+') yields no empty parts
        }
        let part = match part.strip_prefix(b"type") {
            Some(rest) if rest.first().is_some_and(|&c| is_space(c)) => strip_spaces(rest),
            _ => part,
        };
        let n = match_as_name(part).or_else(|| capture_id(part).map(|len| part[..len].to_vec()));
        if let Some(n) = n {
            names.push(Value::String(text::text(&n)));
        }
    }
    names
}

const REGEX_KEYWORDS: &[&[u8]] = &[
    b"return", b"throw", b"case", b"void", b"typeof", b"delete", b"await", b"yield", b"else",
    b"do", b"in", b"of",
];

/// `regex_end(s, i)`: whether the `/` at 1-based `i` opens a regex literal;
/// returns one past the closing flags.
fn regex_end(s: &[u8], i: usize) -> Option<usize> {
    let mut j = i as i64 - 1;
    while j > 0 && at(s, j as usize).is_some_and(is_space) {
        j -= 1;
    }
    let c = at(s, j.max(0) as usize);
    if j > 0 && (ident(c) || c == Some(b')') || c == Some(b']')) {
        let mut k = j;
        while k > 0 && ident(at(s, k as usize)) {
            k -= 1;
        }
        let word = text::sub(s, k + 1, j);
        if !REGEX_KEYWORDS.contains(&word) {
            return None;
        }
    }
    let mut inside = false;
    let mut j = i + 1;
    while j <= s.len() {
        let c = at(s, j).unwrap();
        if c == b'\\' {
            j += 2;
        } else if c == b'\n' {
            return None;
        } else if c == b'/' && !inside {
            j += 1;
            while at(s, j).is_some_and(|c| c.is_ascii_alphabetic()) {
                j += 1;
            }
            return Some(j);
        } else {
            if c == b'[' && !inside {
                inside = true;
            } else if c == b']' && inside {
                inside = false;
            }
            j += 1;
        }
    }
    None
}

fn in_hole(holes: &[(usize, usize)], i: usize) -> bool {
    holes.iter().any(|&(a, b)| i >= a && i < b)
}

/// `mask(s)`: comments blanked; regexes and quoted strings kept as holes.
fn mask(s: &[u8]) -> (Vec<u8>, Vec<(usize, usize)>) {
    let mut out = Vec::new();
    let mut holes = Vec::new();
    let mut i = 1;
    while i <= s.len() {
        let c = at(s, i);
        let nxt = at(s, i + 1);
        if graph::line_comment(c, nxt) {
            i = graph::mask_line_comment(s, &mut out, i, None);
            continue;
        }
        if graph::block_comment(c, nxt) {
            i = graph::mask_block_comment(s, &mut out, i, None);
            continue;
        }
        let mut finish = None;
        if c == Some(b'/') && nxt.is_some() && nxt != Some(b'*') {
            finish = regex_end(s, i);
        } else if matches!(c, Some(b'"') | Some(b'\'') | Some(b'`')) {
            finish = Some(graph::scan_quoted(s, i, c.unwrap(), None));
        }
        if let Some(finish) = finish {
            out.extend_from_slice(&s[i - 1..finish - 1]);
            holes.push((i, finish));
            i = finish;
        } else {
            out.push(c.unwrap());
            i += 1;
        }
    }
    (out, holes)
}

/// `statement_end(s, start, header)`: inclusive end position.
fn statement_end(s: &[u8], start: usize, header: bool) -> usize {
    let (mut paren, mut brace, mut bracket) = (0i64, 0i64, 0i64);
    let mut i = start;
    while i <= s.len() {
        let c = at(s, i).unwrap();
        if c == b'(' {
            paren += 1;
        } else if c == b')' {
            paren = (paren - 1).max(0);
        } else if c == b'[' {
            bracket += 1;
        } else if c == b']' {
            bracket = (bracket - 1).max(0);
        } else if c == b'{' {
            if header && paren == 0 && bracket == 0 && brace == 0 {
                return i - 1;
            }
            brace += 1;
        } else if c == b'}' && !header {
            brace = (brace - 1).max(0);
        } else if paren == 0 && bracket == 0 && brace == 0 {
            if header && (c == b'=' || c == b';') {
                return i - 1;
            } else if c == b';' {
                return i;
            }
        }
        i += 1;
    }
    s.len()
}

/// The `["']([^"']+)["']` tail of a specifier: the first quote of either kind
/// closes the capture, and the capture holds neither kind.
fn spec_capture(rest: &[u8]) -> Option<Vec<u8>> {
    let end = rest.iter().position(|&c| c == b'"' || c == b'\'')?;
    if end == 0 {
        return None;
    }
    Some(rest[..end].to_vec())
}

/// `from_spec(stmt)`: `from%s*["']([^"']+)["']` anywhere.
fn from_spec(stmt: &[u8]) -> Option<Vec<u8>> {
    let mut search = 0;
    while let Some(off) = find_subslice(&stmt[search.min(stmt.len())..], b"from") {
        let p = search + off + 4;
        let after = strip_spaces(&stmt[p..]);
        if let Some(&quote) = after.first()
            && (quote == b'"' || quote == b'\'')
            && let Some(spec) = spec_capture(&after[1..])
        {
            return Some(spec);
        }
        search = search + off + 1;
    }
    None
}

/// `return_type(header)`: the `: Name` after the parameter list.
fn return_type(header: &[u8]) -> Option<Vec<u8>> {
    let open = header.iter().position(|&c| c == b'(')?;
    let mut depth = 1;
    for i in open + 1..header.len() {
        let c = header[i];
        if c == b'(' {
            depth += 1;
        } else if c == b')' {
            depth -= 1;
            if depth == 0 {
                let rest = strip_spaces(&header[i + 1..]);
                let rest = rest.strip_prefix(b":")?;
                let rest = strip_spaces(rest);
                let len = capture_id(rest)?;
                if trim(&rest[len..]).is_empty() {
                    return Some(rest[..len].to_vec());
                }
                return None;
            }
        }
    }
    None
}

fn starts_with_word(s: &[u8], word: &[u8]) -> bool {
    s.starts_with(word) && !s.get(word.len()).is_some_and(|&c| is_ts_ident(c))
}

/// `export_fact(stmt, header)`.
fn export_fact(stmt: &[u8], header: &[u8]) -> Option<Value> {
    // `^export%s+type%s*{` or `^export%s+%*`
    if match_export_type_star(stmt) {
        return Some(record(vec![(
            "spec",
            from_spec(stmt)
                .map(|s| Value::String(text::text(&s)))
                .unwrap_or(Value::Nil),
        )]));
    }
    if match_prefix_brace(stmt, b"export") {
        return Some(record(vec![
            (
                "spec",
                from_spec(stmt)
                    .map(|s| Value::String(text::text(&s)))
                    .unwrap_or(Value::Nil),
            ),
            ("ops", Value::Vector(brace_names(stmt))),
        ]));
    }
    // tail = stmt minus `^export%s+` minus `^default%s+`
    let mut tail = strip_prefix_word(stmt, b"export").unwrap_or(stmt);
    tail = strip_prefix_word(tail, b"default").unwrap_or(tail);
    if let Some(rest) = strip_prefix_word(tail, b"async") {
        if !starts_with_word(rest, b"function") {
            return None;
        }
        tail = rest;
    }
    let mut n: Option<Vec<u8>> = None;
    let mut typ: Option<Vec<u8>> = None;
    if starts_with_word(tail, b"function") {
        // `^function%s+%*?%s*(ID)`
        let rest = strip_spaces(&tail[8..]);
        let rest = rest.strip_prefix(b"*").map(strip_spaces).unwrap_or(rest);
        // NOTE: `%*?` consumes the star only when directly after %s+.
        let rest = if tail[8..].iter().take_while(|&&c| is_space(c)).count() > 0 {
            rest
        } else {
            return None;
        };
        n = capture_id(rest).map(|len| rest[..len].to_vec());
        typ = return_type(header);
    } else if starts_with_word(tail, b"class") {
        let rest = strip_spaces(&tail[5..]);
        if tail[5..].iter().take_while(|&&c| is_space(c)).count() > 0 {
            n = capture_id(rest).map(|len| rest[..len].to_vec());
        }
    } else if starts_with_word(tail, b"interface") {
        return Some(record(vec![("interface", Value::Bool(true))]));
    } else if starts_with_word(tail, b"enum") {
        let rest = strip_spaces(&tail[4..]);
        if tail[4..].iter().take_while(|&&c| is_space(c)).count() > 0 {
            n = capture_id(rest).map(|len| rest[..len].to_vec());
        }
    } else if starts_with_word(strip_prefix_word(stmt, b"export").unwrap_or(stmt), b"const")
        || starts_with_word(strip_prefix_word(stmt, b"export").unwrap_or(stmt), b"let")
        || starts_with_word(strip_prefix_word(stmt, b"export").unwrap_or(stmt), b"var")
    {
        // `^%a+%s+(ID)` on the tail
        let letters = tail
            .iter()
            .take_while(|&&c| c.is_ascii_alphabetic())
            .count();
        let rest = &tail[letters..];
        let spaces = rest.iter().take_while(|&&c| is_space(c)).count();
        if letters > 0 && spaces > 0 {
            let rest = &rest[spaces..];
            n = capture_id(rest).map(|len| rest[..len].to_vec());
        }
        typ = ["const", "let", "var"]
            .iter()
            .find_map(|k| match_const_typed(header, k.as_bytes()));
    }
    n.map(|n| {
        let mut entries = vec![
            ("ops", Value::Vector(vec![Value::String(text::text(&n))])),
            ("value", Value::Bool(true)),
        ];
        if let Some(typ) = typ {
            entries.push(("type", Value::String(text::text(&typ))));
        }
        record(entries)
    })
}

/// `^export%s+type%s*{` or `^export%s+%*`.
fn match_export_type_star(stmt: &[u8]) -> bool {
    let Some(rest) = strip_prefix_word(stmt, b"export") else {
        return false;
    };
    if rest.first() == Some(&b'*') {
        return true; // `%s*%*`: the strip consumed the required spaces
    }
    let Some(rest) = strip_prefix_word(rest, b"type") else {
        return false;
    };
    // `type%s*{`: spaces then `{` — strip_prefix_word required spaces already.
    let rest = strip_spaces(rest);
    rest.first() == Some(&b'{')
}

/// `^export%s+{`: export, spaces, then `{`.
fn match_prefix_brace(stmt: &[u8], word: &[u8]) -> bool {
    match strip_prefix_word(stmt, word) {
        Some(rest) => strip_spaces(rest).first() == Some(&b'{'),
        None => false,
    }
}

/// `^word%s+` — strips the word and the following whitespace run.
fn strip_prefix_word<'a>(s: &'a [u8], word: &[u8]) -> Option<&'a [u8]> {
    let rest = s.strip_prefix(word)?;
    let spaces = rest.iter().take_while(|&&c| is_space(c)).count();
    if spaces == 0 {
        return None;
    }
    Some(&rest[spaces..])
}

/// `%f[%w_$]<kw>%s+ID%s*:%s*(ID)%s*$` on the header.
fn match_const_typed(header: &[u8], keyword: &[u8]) -> Option<Vec<u8>> {
    let mut search = 0;
    while let Some(off) = find_subslice(&header[search.min(header.len())..], keyword) {
        let p = search + off;
        let before_ok = p == 0 || !is_ts_ident(header[p - 1]);
        let rest = &header[p + keyword.len()..];
        let spaces = rest.iter().take_while(|&&c| is_space(c)).count();
        if before_ok && spaces > 0 {
            let rest = &rest[spaces..];
            if let Some(len) = capture_id(rest) {
                let rest = strip_spaces(&rest[len..]);
                if let Some(rest) = rest.strip_prefix(b":") {
                    let rest = strip_spaces(rest);
                    if let Some(tlen) = capture_id(rest)
                        && trim(&rest[tlen..]).is_empty()
                    {
                        return Some(rest[..tlen].to_vec());
                    }
                }
            }
        }
        search = p + keyword.len();
    }
    None
}

fn find_subslice(s: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || s.len() < needle.len() {
        return None;
    }
    s.windows(needle.len()).position(|w| w == needle)
}

/// `^%s*word%f[^%w_$]` anchored at 1-based `start`; returns (a, b) inclusive.
fn find_anchored(text: &[u8], word: &[u8], start: usize) -> Option<(usize, usize)> {
    let mut p = start;
    while at(text, p).is_some_and(is_space) {
        p += 1;
    }
    if text::sub(text, p as i64, p as i64 + word.len() as i64 - 1) != word {
        return None;
    }
    if at(text, p + word.len()).is_some_and(is_ts_ident) {
        return None;
    }
    Some((start, p + word.len() - 1))
}

/// `read_module(source)`: imports, exports and `invoke("...")` commands.
pub fn read_module(source: &[u8]) -> Value {
    let (text, holes) = mask(source);
    let mut imports = Vec::new();
    let mut exports = Vec::new();
    let mut invokes: Vec<Value> = Vec::new();
    for word in ["import", "export"] {
        let mut start = 1usize;
        while start <= text.len() {
            if let Some((a, b)) = find_anchored(&text, word.as_bytes(), start) {
                if !in_hole(&holes, a) {
                    let at_pos = text::find_plain(&text, word.as_bytes(), a).unwrap();
                    let stmt = text::sub(
                        &text,
                        at_pos as i64,
                        statement_end(&text, at_pos, false) as i64,
                    );
                    let header = text::sub(
                        &text,
                        at_pos as i64,
                        statement_end(&text, at_pos, true) as i64,
                    );
                    if word == "import" {
                        let spec = from_spec(stmt).or_else(|| match_bare_import(stmt));
                        if let Some(spec) = spec {
                            let mut names = brace_names(stmt);
                            let tail = strip_prefix_word(stmt, b"import")
                                .map(|t| strip_prefix_word(t, b"type").unwrap_or(t))
                                .unwrap_or(stmt);
                            if let Some(len) = capture_id(tail) {
                                let n = &tail[..len];
                                let rest = &tail[len..];
                                let rest = strip_spaces(rest);
                                let comma = rest.first() == Some(&b',');
                                let from_after = {
                                    // `^ID%s+from%f[^%w_$]` on the whole tail
                                    let after = strip_spaces(&tail[len..]);
                                    after.starts_with(b"from")
                                        && !after.get(4).is_some_and(|&c| is_ts_ident(c))
                                        && tail[len..].iter().take_while(|&&c| is_space(c)).count()
                                            > 0
                                };
                                if n != b"type" && (comma || from_after) {
                                    names.push(Value::String(text::text(n)));
                                }
                            }
                            if let Some(n) = match_star_as(stmt) {
                                names.push(Value::String(text::text(&n)));
                            }
                            imports.push(record(vec![
                                ("spec", Value::String(text::text(&spec))),
                                ("names", Value::Vector(names)),
                            ]));
                        }
                    } else if let Some(fact) = export_fact(stmt, header) {
                        exports.push(fact);
                    }
                }
                start = b + 1;
            }
            match text::find_plain(&text, b"\n", start) {
                Some(newline) => start = newline + 1,
                None => break,
            }
        }
    }
    let mut i = 1;
    while let Some(at_pos) = text::find_plain(&text, b"invoke", i) {
        i = at_pos + 1;
        if ident(at(&text, at_pos.wrapping_sub(1)))
            || ident(at(&text, at_pos + 6))
            || in_hole(&holes, at_pos)
        {
            continue;
        }
        let mut j = at_pos + 6;
        while at(&text, j).is_some_and(is_space) {
            j += 1;
        }
        if at(&text, j) == Some(b'<') {
            let mut depth = 1;
            j += 1;
            while j <= text.len() && depth > 0 {
                match at(&text, j).unwrap() {
                    b'<' => depth += 1,
                    b'>' => depth -= 1,
                    _ => {}
                }
                j += 1;
            }
        }
        if at(&text, j) == Some(b'(') {
            j += 1;
            while at(&text, j).is_some_and(is_space) {
                j += 1;
            }
            let quote = at(&text, j);
            if quote == Some(b'"') || quote == Some(b'\'') {
                let quote = quote.unwrap();
                let mut stop = j + 1;
                while stop <= text.len() {
                    let c = at(&text, stop).unwrap();
                    if c == b'\\' {
                        stop += 2;
                    } else if c == quote {
                        break;
                    } else {
                        stop += 1;
                    }
                }
                let end = stop.min(text.len() + 1) - 1;
                let v = Value::String(text::text(text::sub(&text, (j + 1) as i64, end as i64)));
                if !invokes.contains(&v) {
                    invokes.push(v);
                }
            }
        }
    }
    record(vec![
        ("imports", Value::Vector(imports)),
        ("exports", Value::Vector(exports)),
        ("invokes", Value::Vector(invokes)),
    ])
}

/// `^import%s*["']([^"']+)["']` — a side-effect import.
fn match_bare_import(stmt: &[u8]) -> Option<Vec<u8>> {
    let rest = stmt.strip_prefix(b"import")?;
    let rest = strip_spaces(rest);
    let quote = *rest.first()?;
    if quote != b'"' && quote != b'\'' {
        return None;
    }
    spec_capture(&rest[1..])
}

/// `%*%s+as%s+(ID)` anywhere in the statement.
fn match_star_as(stmt: &[u8]) -> Option<Vec<u8>> {
    let mut search = 0;
    while let Some(off) = find_subslice(&stmt[search.min(stmt.len())..], b"*") {
        let p = search + off;
        let rest = strip_spaces(&stmt[p + 1..]);
        if let Some(rest) = rest.strip_prefix(b"as") {
            let spaces = rest.iter().take_while(|&&c| is_space(c)).count();
            if spaces > 0 {
                let rest = &rest[spaces..];
                if let Some(len) = capture_id(rest) {
                    return Some(rest[..len].to_vec());
                }
            }
        }
        search = p + 1;
    }
    None
}

/// The 12 `%.[cm]?[jt]sx?$` suffixes, longest first.
const TS_EXTENSIONS: &[&str] = &[
    ".cjsx", ".ctsx", ".mjsx", ".mtsx", ".cjs", ".cts", ".mjs", ".mts", ".jsx", ".tsx", ".js",
    ".ts",
];

/// `accepts(relative)` — the full path is passed, like the Lua scanner.
fn accepts(path: &str) -> bool {
    if !TS_EXTENSIONS.iter().any(|ext| path.ends_with(ext)) {
        return false;
    }
    if path.ends_with(".d.ts") {
        return false;
    }
    // `%.test%.[cm]?[jt]sx?$` and `%.spec%.[cm]?[jt]sx?$`
    for marker in [".test", ".spec"] {
        for ext in TS_EXTENSIONS {
            if path.ends_with(&format!("{marker}{ext}")) {
                return false;
            }
        }
    }
    let slashed = format!("/{path}");
    !(slashed.contains("/node_modules/")
        || slashed.contains("/dist/")
        || slashed.contains("/target/"))
}

/// `normalize(path)`: resolve `.` and `..` segments.
fn normalize(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for p in path.split('/') {
        if p.is_empty() {
            continue;
        }
        if p == ".." {
            parts.pop();
        } else if p != "." {
            parts.push(p);
        }
    }
    let mut out = String::new();
    if path.starts_with('/') {
        out.push('/');
    }
    out.push_str(&parts.join("/"));
    out
}

/// `module_relative(path, root)`.
fn module_relative(path: &str, root: &str) -> String {
    let rel = graph::relative_path(path, root);
    // `gsub('%.[cm]?[jt]sx?$','')`
    let rel = strip_ts_extension(&rel);
    let rel = if rel == "index" {
        String::new()
    } else {
        rel.strip_suffix("/index")
            .map(|s| format!("{s}/"))
            .unwrap_or(rel)
    };
    let rel = rel.replace("/.", ".");
    let rel = rel.strip_suffix('/').unwrap_or(&rel).to_owned();
    rel.replace('/', ".")
}

fn strip_ts_extension(rel: &str) -> String {
    for ext in TS_EXTENSIONS {
        if let Some(stripped) = rel.strip_suffix(ext) {
            return stripped.to_owned();
        }
    }
    rel.to_owned()
}

const EXTS: &[&str] = &[".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs"];

pub struct TypeScriptScanner;

pub static SCANNER: TypeScriptScanner = TypeScriptScanner;

impl Scanner for TypeScriptScanner {
    fn accepts(&self, relative: &str, _opts: &Value) -> bool {
        accepts(relative)
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
        let mut sorted: Vec<&crate::files::SourceFile> =
            files.iter().filter(|f| accepts(&f.path)).collect();
        sorted.sort_by(|a, b| a.path.cmp(&b.path));
        let mut by_path: HashMap<String, Value> = HashMap::new();
        let mut project: HashSet<String> = HashSet::new();
        for f in &sorted {
            let cid = graph::id_of(
                &graph::ns_join(&ns_prefix, &module_relative(&f.path, root)),
                Some(&Value::String(prefix.clone())),
            );
            by_path.insert(f.path.clone(), cid.clone());
            project.insert(crate::value::encode_key(&cid));
        }
        let resolve = |file: &str, spec: &Value| -> Option<Value> {
            let spec = match spec {
                Value::String(s) => s.clone(),
                _ => return None,
            };
            if !spec.starts_with('.') {
                return None;
            }
            let dir = match file.rsplit_once('/') {
                Some((d, _)) => d,
                None => "",
            };
            let base = normalize(&format!("{dir}/{spec}"));
            if let Some(cid) = by_path.get(&base) {
                return Some(cid.clone());
            }
            for ext in EXTS {
                if let Some(cid) = by_path.get(&format!("{base}{ext}")) {
                    return Some(cid.clone());
                }
            }
            for ext in EXTS {
                if let Some(cid) = by_path.get(&format!("{base}/index{ext}")) {
                    return Some(cid.clone());
                }
            }
            None
        };
        let mut classes = Vec::new();
        let mut edges: Vec<Value> = Vec::new();
        let mut foreigns: Vec<Value> = Vec::new();
        let mut invokes_map: Vec<(Value, Value)> = Vec::new();
        for f in &sorted {
            let surface = read_module(&f.source);
            let cid = by_path[&f.path].clone();
            let mut ops: Vec<Value> = Vec::new();
            let mut bindings: Vec<(String, Value)> = Vec::new();
            let mut specs: Vec<Value> = Vec::new();
            let mut impls: Vec<Value> = Vec::new();
            let mut interface = false;
            let mut value = false;
            for imp in seq(get(&surface, "imports")) {
                let spec = get(imp, "spec").cloned().unwrap_or(Value::Nil);
                if !specs.contains(&spec) {
                    specs.push(spec.clone());
                }
                if let Some(to) = resolve(&f.path, &spec) {
                    for n in seq(get(imp, "names")) {
                        if let Value::String(n) = n {
                            bindings.push((n.clone(), to.clone()));
                        }
                    }
                }
            }
            for exp in seq(get(&surface, "exports")) {
                for n in seq(get(exp, "ops")) {
                    if let Value::String(n) = n {
                        let op = record(vec![
                            ("name", Value::String(n.clone())),
                            ("text", Value::String(n.clone())),
                        ]);
                        if !ops.contains(&op) {
                            ops.push(op);
                        }
                    }
                }
                if let Some(spec) = get(exp, "spec")
                    && !specs.contains(spec)
                {
                    specs.push(spec.clone());
                }
                if let Some(Value::String(typ)) = get(exp, "type")
                    && let Some((_, to)) = bindings.iter().find(|(n, _)| n == typ)
                    && !impls.contains(to)
                {
                    impls.push(to.clone());
                }
                interface = interface || flag(exp, "interface");
                value = value || flag(exp, "value");
            }
            let mut c = record(vec![
                ("id", cid.clone()),
                ("name", Value::String(graph::module_name(&cid))),
                (
                    "ns",
                    Value::String(graph::ns_join(&ns_prefix, &module_relative(&f.path, root))),
                ),
                ("lang", Value::keyword("typescript")),
                ("file", Value::String(graph::relative_path(&f.path, cwd))),
            ]);
            if !ops.is_empty() {
                edn::record::put(&mut c, "ops", Value::Vector(ops));
            }
            if interface && !value {
                edn::record::put(&mut c, "stereotype", Value::keyword("interface"));
            }
            classes.push(c);
            let mut requires: Vec<Value> = Vec::new();
            let mut external: Vec<Value> = Vec::new();
            const ASSETS: &[&str] = &[
                "css", "scss", "sass", "less", "svg", "png", "jpg", "jpeg", "gif", "webp", "html",
                "md", "json",
            ];
            for spec in &specs {
                let spec_text = match spec {
                    Value::String(s) => s.clone(),
                    _ => continue,
                };
                let ext = spec_text.rsplit('.').next().unwrap_or("");
                if ASSETS.contains(&ext) {
                    continue;
                }
                if let Some(to) = resolve(&f.path, spec) {
                    if !requires.contains(&to) {
                        requires.push(to);
                    }
                } else if !spec_text.starts_with('.') {
                    let foreign = Value::keyword(
                        &spec_text
                            .strip_prefix('@')
                            .map(|s| format!("at.{s}"))
                            .unwrap_or(spec_text.clone())
                            .replace([':', '/'], "."),
                    );
                    if !external.contains(&foreign) {
                        external.push(foreign.clone());
                    }
                    if !foreigns.contains(&foreign) {
                        foreigns.push(foreign);
                    }
                }
            }
            for (group, kind) in [
                (&requires, "dependency"),
                (&external, "dependency"),
                (&impls, "implements"),
            ] {
                for to in group {
                    if cid != *to {
                        let e = record(vec![
                            ("from", cid.clone()),
                            ("to", to.clone()),
                            ("kind", Value::keyword(kind)),
                        ]);
                        if !edges.contains(&e) {
                            edges.push(e);
                        }
                    }
                }
            }
            let inv = seq(get(&surface, "invokes"));
            if !inv.is_empty() {
                invokes_map.push((cid.clone(), Value::Vector(inv.to_vec())));
            }
        }
        for cid in foreigns {
            if !project.contains(&crate::value::encode_key(&cid)) {
                classes.push(graph::foreign_class(&cid));
            }
        }
        Ok(record(vec![
            ("classes", Value::Vector(classes)),
            ("edges", Value::Vector(edges)),
            ("invokes", Value::Map(invokes_map)),
        ]))
    }
}
