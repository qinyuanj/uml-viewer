//! Port of `love/uml/rust_language/graph_rust.lua`: canonical file snapshots
//! are supplied by the IO adapter. Scanner limitations are preserved.

use std::collections::{HashMap, HashSet};

use edn::Value;

use crate::graph::{self, Scanner};
use crate::text::{self, at, is_space};
use crate::value::get_truthy as get;
use edn::record::{record, seq};

fn is_rust_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

fn is_rust_ident(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

fn in_hole(holes: &[(usize, usize)], i: usize) -> bool {
    holes.iter().any(|&(a, b)| i >= a && i < b)
}

/// `mask(source)`: comments blanked; strings and char literals kept as holes.
fn mask(source: &[u8]) -> (Vec<u8>, Vec<(usize, usize)>) {
    let mut out = Vec::new();
    let mut holes = Vec::new();
    let mut i = 1;
    while i <= source.len() {
        let c = at(source, i);
        let n = at(source, i + 1);
        if graph::line_comment(c, n) {
            i = graph::mask_line_comment(source, &mut out, i, None);
            continue;
        }
        if graph::block_comment(c, n) {
            i = graph::mask_block_comment(source, &mut out, i, None);
            continue;
        }
        let mut finish = None;
        if c == Some(b'"') {
            finish = Some(graph::scan_quoted(source, i, b'"', None));
        } else if c == Some(b'\'')
            && n.is_some()
            && n != Some(b'\\')
            && at(source, i + 2) == Some(b'\'')
        {
            finish = Some(i + 3);
        } else if c == Some(b'\'') && n == Some(b'\\') {
            finish = Some(text::find_plain(source, b"'", i + 3).unwrap_or(source.len()) + 1);
        }
        if let Some(finish) = finish {
            holes.push((i, finish));
            out.extend_from_slice(&source[i - 1..finish - 1]);
            i = finish;
        } else {
            out.push(c.unwrap());
            i += 1;
        }
    }
    (out, holes)
}

/// `paths(path)`: strip `{...}` groups and a trailing `as` rename, then split
/// on `::` into trimmed non-empty segments.
fn paths(path: &[u8]) -> Vec<Vec<u8>> {
    // gsub('{[^\n]*', ''): from each '{' to before the next newline.
    let mut stripped = Vec::with_capacity(path.len());
    let mut i = 0;
    while i < path.len() {
        if path[i] == b'{' {
            while i < path.len() && path[i] != b'\n' {
                i += 1;
            }
        } else {
            stripped.push(path[i]);
            i += 1;
        }
    }
    // gsub(' as [^\n]*$', '')
    let mut end = stripped.len();
    if let Some(pos) = find_last_as(&stripped) {
        end = pos;
    }
    let stripped = &stripped[..end];
    let mut xs = Vec::new();
    let mut rest = stripped;
    loop {
        match find_subslice(rest, b"::") {
            Some(p) => {
                let seg = text::trim(&rest[..p]);
                if !seg.is_empty() {
                    xs.push(seg.to_vec());
                }
                rest = &rest[p + 2..];
            }
            None => {
                let seg = text::trim(rest);
                if !seg.is_empty() {
                    xs.push(seg.to_vec());
                }
                break;
            }
        }
    }
    xs
}

/// The ` as [^\n]*$` strip: last ` as ` whose remainder has no newline.
fn find_last_as(s: &[u8]) -> Option<usize> {
    let mut found = None;
    let mut start = 0;
    while let Some(off) = find_subslice(&s[start.min(s.len())..], b" as ") {
        let pos = start + off;
        if !s[pos..].contains(&b'\n') {
            found = Some(pos);
        }
        start = pos + 1;
    }
    found
}

fn find_subslice(s: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || s.len() < needle.len() {
        return None;
    }
    s.windows(needle.len()).position(|w| w == needle)
}

/// `%b()` at the start of `s`: length of the balanced `(...)` span.
fn balanced_parens(s: &[u8]) -> Option<usize> {
    if s.first() != Some(&b'(') {
        return None;
    }
    let mut depth = 0;
    for (i, &c) in s.iter().enumerate() {
        match c {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i + 1);
                }
            }
            _ => {}
        }
    }
    None
}

/// `^%s*pub%b()%s+` then `^%s*pub%s+` then (for `command`) `^%s*async%s+`.
fn strip_pub(s: &[u8], also_async: bool) -> &[u8] {
    let spaces = s.iter().take_while(|&&c| is_space(c)).count();
    if let Some(rest) = s[spaces..].strip_prefix(b"pub")
        && let Some(len) = balanced_parens(rest)
    {
        let after = &rest[len..];
        let trailing = after.iter().take_while(|&&c| is_space(c)).count();
        if trailing > 0 {
            return strip_pub_simple(&after[trailing..], also_async);
        }
    }
    strip_pub_simple(s, also_async)
}

fn strip_pub_simple(s: &[u8], also_async: bool) -> &[u8] {
    let spaces = s.iter().take_while(|&&c| is_space(c)).count();
    if let Some(rest) = s[spaces..].strip_prefix(b"pub") {
        let trailing = rest.iter().take_while(|&&c| is_space(c)).count();
        if trailing > 0 {
            let after = &rest[trailing..];
            if !also_async {
                return after;
            }
            return strip_async(after);
        }
    }
    if also_async {
        return strip_async(s);
    }
    s
}

fn strip_async(s: &[u8]) -> &[u8] {
    let spaces = s.iter().take_while(|&&c| is_space(c)).count();
    if let Some(rest) = s[spaces..].strip_prefix(b"async") {
        let trailing = rest.iter().take_while(|&&c| is_space(c)).count();
        if trailing > 0 {
            return &rest[trailing..];
        }
    }
    s
}

/// `^#%[%s*tauri%s*::%s*command%f[^%w_][^%]]*%](.*)$`: returns the capture.
fn match_command_attribute(s: &[u8]) -> Option<&[u8]> {
    let s = s.strip_prefix(b"#[")?;
    let s = &s[s.iter().take_while(|&&c| is_space(c)).count()..];
    let s = s.strip_prefix(b"tauri")?;
    let s = &s[s.iter().take_while(|&&c| is_space(c)).count()..];
    let s = s.strip_prefix(b"::")?;
    let s = &s[s.iter().take_while(|&&c| is_space(c)).count()..];
    let s = s.strip_prefix(b"command")?;
    if s.first().is_some_and(|&c| crate::text::is_word(c)) {
        return None;
    }
    let close = s.iter().position(|&c| c == b']')?;
    Some(&s[close + 1..])
}

/// `command(text, at)`: a `#[tauri::command]` attribute directly precedes the
/// `fn` at `at` (within the 240-byte lookback), modulo pub/async.
fn command(text: &[u8], at: usize) -> bool {
    let start = at.saturating_sub(240).max(1);
    let prefix = text::sub(text, start as i64, at as i64 - 1);
    let mut search = 0;
    while let Some(off) = find_subslice(&prefix[search..], b"#[") {
        let p = search + off; // 0-based position of '#['
        if let Some(rest) = match_command_attribute(&prefix[p..]) {
            let rest = strip_pub(rest, true);
            if rest.iter().all(|&c| is_space(c)) {
                return true;
            }
        }
        search = p + 2;
    }
    false
}

/// Positions of `word` with `%f[%w_]` before and `%f[^%w_]` after (1-based).
fn word_positions(text: &[u8], word: &[u8]) -> Vec<usize> {
    let mut out = Vec::new();
    let mut search = 0;
    while let Some(off) = find_subslice(&text[search.min(text.len())..], word) {
        let p = search + off; // 0-based
        let before_ok = p == 0 || !crate::text::is_word(text[p - 1]);
        let after = p + word.len();
        let after_ok = after >= text.len() || !crate::text::is_word(text[after]);
        if before_ok && after_ok {
            out.push(p + 1);
        }
        search = after.max(p + 1);
    }
    out
}

/// `read_module(source)`.
pub fn read_module(source: &[u8]) -> Value {
    let (text, holes) = mask(source);
    let mut uses: Vec<Vec<u8>> = Vec::new();
    let mut mods: Vec<Vec<u8>> = Vec::new();
    let mut impls: Vec<Vec<u8>> = Vec::new();
    let mut commands: Vec<Vec<u8>> = Vec::new();
    let mut ops: Vec<Vec<u8>> = Vec::new();
    let mut bindings: Vec<(Vec<u8>, Vec<Vec<u8>>)> = Vec::new();
    let mut publics: HashSet<Vec<u8>> = HashSet::new();
    let mut use_end = 0usize;
    let mut mod_end = 0usize;
    for i in 1..=text.len() {
        if i == 1 || at(&text, i - 1) == Some(b'\n') {
            let rest = &text[i - 1..];
            let body = strip_pub(rest, false);
            let u = match_use(body);
            let m = match_mod(body);
            if let Some(capture) = u
                && i > use_end
            {
                use_end = i + find_subslice(rest, b";").unwrap_or(rest.len() - 1);
                if !in_hole(&holes, i) {
                    uses.push(text::trim(capture).to_vec());
                }
            }
            if let Some(capture) = m
                && i > mod_end
            {
                mod_end = i + find_subslice(rest, b";").unwrap_or(rest.len() - 1);
                if !in_hole(&holes, i) && capture != b"tests" {
                    mods.push(capture.to_vec());
                }
            }
        }
    }
    for at_pos in word_positions(&text, b"pub") {
        if !in_hole(&holes, at_pos)
            && let Some(name) = match_pub_fn(&text[at_pos - 1 + 3..])
        {
            publics.insert(name);
        }
    }
    let mut functions: Vec<Vec<u8>> = Vec::new();
    for at_pos in word_positions(&text, b"fn") {
        if !in_hole(&holes, at_pos)
            && let Some(name) = match_fn_name(&text[at_pos - 1..])
        {
            functions.push(name.clone());
            if command(&text, at_pos) && !commands.contains(&name) {
                commands.push(name);
            }
        }
    }
    for n in &functions {
        if publics.contains(n) {
            if !ops.contains(n) {
                ops.push(n.clone());
            }
        } else {
            for cmd in &commands {
                if cmd == n && !ops.contains(n) {
                    ops.push(n.clone());
                }
            }
        }
    }
    for at_pos in word_positions(&text, b"impl") {
        if !in_hole(&holes, at_pos) {
            let rest = &text[at_pos - 1 + 4..];
            let mut tr = match_trait_for(rest);
            if starts_with_spaces_then(rest, b"<") {
                let generic: &[u8] = match_generic(rest).unwrap_or(&[]);
                let offset = find_subslice(rest, b"<").unwrap();
                let mut search = 0;
                while let Some(off) = find_subslice(&generic[search.min(generic.len())..], b">") {
                    let close = search + off; // 0-based in generic
                    if let Some(candidate) = match_trait_for(&rest[offset + close + 2..]) {
                        tr = Some(candidate);
                    }
                    search = close + 1;
                }
            }
            if let Some(tr) = tr
                && find_subslice(&tr, b":::").is_none()
                && tr.last() != Some(&b':')
            {
                impls.push(tr);
            }
        }
    }
    for u in &uses {
        let segs = paths(u);
        if matches!(
            segs.first().map(Vec::as_slice),
            Some(b"crate" | b"self" | b"super")
        ) && segs.len() > 1
        {
            let last = segs.last().unwrap().clone();
            bindings.retain(|(k, _)| k != &last);
            bindings.push((last, segs));
        }
    }
    record(vec![
        (
            "uses",
            Value::Vector(uses.iter().map(|u| Value::String(text::text(u))).collect()),
        ),
        (
            "mods",
            Value::Vector(mods.iter().map(|m| Value::String(text::text(m))).collect()),
        ),
        (
            "impls",
            Value::Vector(impls.iter().map(|t| Value::String(text::text(t))).collect()),
        ),
        (
            "bindings",
            Value::Map(
                bindings
                    .iter()
                    .map(|(k, segs)| {
                        (
                            Value::String(text::text(k)),
                            Value::Vector(
                                segs.iter().map(|s| Value::String(text::text(s))).collect(),
                            ),
                        )
                    })
                    .collect(),
            ),
        ),
        (
            "commands",
            Value::Vector(
                commands
                    .iter()
                    .map(|c| Value::String(text::text(c)))
                    .collect(),
            ),
        ),
        (
            "ops",
            Value::Vector(ops.iter().map(|o| Value::String(text::text(o))).collect()),
        ),
    ])
}

/// `^%s*use%s+([^;]+);` — the capture may span lines.
fn match_use(body: &[u8]) -> Option<&[u8]> {
    let s = &body[body.iter().take_while(|&&c| is_space(c)).count()..];
    let s = s.strip_prefix(b"use")?;
    let spaces = s.iter().take_while(|&&c| is_space(c)).count();
    if spaces == 0 {
        return None;
    }
    let s = &s[spaces..];
    let end = s.iter().position(|&c| c == b';')?;
    if end == 0 {
        return None;
    }
    Some(&s[..end])
}

/// `^%s*mod%s+([A-Za-z_][A-Za-z0-9_]*)%s*;`.
fn match_mod(body: &[u8]) -> Option<&[u8]> {
    let s = &body[body.iter().take_while(|&&c| is_space(c)).count()..];
    let s = s.strip_prefix(b"mod")?;
    let spaces = s.iter().take_while(|&&c| is_space(c)).count();
    if spaces == 0 {
        return None;
    }
    let s = &s[spaces..];
    if !s.first().is_some_and(|&c| is_rust_ident_start(c)) {
        return None;
    }
    let len = s.iter().take_while(|&&c| is_rust_ident(c)).count();
    let name = &s[..len];
    let s = &s[len..];
    let s = &s[s.iter().take_while(|&&c| is_space(c)).count()..];
    s.strip_prefix(b";")?;
    Some(name)
}

/// `rest:gsub('^(%s+)async%s+','%1')` then `^%s+fn%s+([A-Za-z_][A-Za-z0-9_]*)`
/// for the `pub` scan.
fn match_pub_fn(s: &[u8]) -> Option<Vec<u8>> {
    let lead = s.iter().take_while(|&&c| is_space(c)).count();
    if lead == 0 {
        return None;
    }
    let mut rest = &s[lead..];
    if let Some(after) = rest.strip_prefix(b"async") {
        let trailing = after.iter().take_while(|&&c| is_space(c)).count();
        if trailing > 0 {
            rest = &after[trailing..];
        }
    }
    let rest = rest.strip_prefix(b"fn")?;
    let spaces = rest.iter().take_while(|&&c| is_space(c)).count();
    if spaces == 0 {
        return None;
    }
    let rest = &rest[spaces..];
    if !rest.first().is_some_and(|&c| is_rust_ident_start(c)) {
        return None;
    }
    let len = rest.iter().take_while(|&&c| is_rust_ident(c)).count();
    Some(rest[..len].to_vec())
}

/// `^fn%s+([A-Za-z_][A-Za-z0-9_]*)` — text starting at the `fn` keyword.
fn match_fn_name(s: &[u8]) -> Option<Vec<u8>> {
    let s = s.strip_prefix(b"fn")?;
    let spaces = s.iter().take_while(|&&c| is_space(c)).count();
    if spaces == 0 {
        return None;
    }
    let s = &s[spaces..];
    if !s.first().is_some_and(|&c| is_rust_ident_start(c)) {
        return None;
    }
    let len = s.iter().take_while(|&&c| is_rust_ident(c)).count();
    Some(s[..len].to_vec())
}

/// `^%s+([A-Za-z_][A-Za-z0-9_:]*)%s+for%s+`.
fn match_trait_for(s: &[u8]) -> Option<Vec<u8>> {
    let spaces = s.iter().take_while(|&&c| is_space(c)).count();
    if spaces == 0 {
        return None;
    }
    let s = &s[spaces..];
    if !s.first().is_some_and(|&c| is_rust_ident_start(c)) {
        return None;
    }
    let len = s
        .iter()
        .take_while(|&&c| is_rust_ident(c) || c == b':')
        .count();
    let name = &s[..len];
    let s = &s[len..];
    let spaces = s.iter().take_while(|&&c| is_space(c)).count();
    if spaces == 0 {
        return None;
    }
    let s = &s[spaces..];
    let s = s.strip_prefix(b"for")?;
    if s.first().is_some_and(|&c| is_space(c)) {
        Some(name.to_vec())
    } else {
        None
    }
}

fn starts_with_spaces_then(s: &[u8], lit: &[u8]) -> bool {
    let spaces = s.iter().take_while(|&&c| is_space(c)).count();
    s[spaces..].starts_with(lit)
}

/// `^%s*<([^;{]*)` — the generic parameter text after `<`.
fn match_generic(s: &[u8]) -> Option<&[u8]> {
    let spaces = s.iter().take_while(|&&c| is_space(c)).count();
    let s = &s[spaces..];
    let s = s.strip_prefix(b"<")?;
    let end = s
        .iter()
        .position(|&c| c == b';' || c == b'{')
        .unwrap_or(s.len());
    Some(&s[..end])
}

fn parent(path: &str) -> Option<String> {
    // `^(.*)/[^/]+$`: a slash with a non-empty final segment.
    match path.rsplit_once('/') {
        Some((p, last)) if !last.is_empty() => Some(p.to_owned()),
        _ => None,
    }
}

pub struct RustScanner;

pub static SCANNER: RustScanner = RustScanner;

/// `metadata_paths(root)`: the explicitly named Cargo.toml files.
fn metadata_paths(root: &str) -> Vec<String> {
    let mut out = vec![format!("{root}/Cargo.toml")];
    if let Some(p) = parent(root) {
        out.push(format!("{p}/Cargo.toml"));
    }
    out
}

/// `lib_name(root, by_path)`: the `[lib] name = "..."` of the first present
/// metadata file.
fn lib_name(root: &str, by_path: &HashMap<String, &crate::files::SourceFile>) -> Option<Vec<u8>> {
    let mut cargo: Option<&[u8]> = None;
    for path in metadata_paths(root) {
        if let Some(f) = by_path.get(&path) {
            cargo = Some(&f.source);
            break;
        }
    }
    let cargo = cargo?;
    let b = find_subslice(cargo, b"[lib]").map(|a| a + 5)?;
    let finish = find_subslice(&cargo[b..], b"\n[")
        .map(|p| b + p)
        .unwrap_or(cargo.len());
    let body = &cargo[b..finish];
    // `^name%s*=%s*"([^"]+)"` or `\nname%s*=%s*"([^"]+)"`
    let mut search = 0;
    while search <= body.len() {
        let at_line_start = search == 0 || body[search - 1] == b'\n';
        if at_line_start && body[search..].starts_with(b"name") {
            let s = &body[search + 4..];
            let s = &s[s.iter().take_while(|&&c| is_space(c)).count()..];
            if let Some(s) = s.strip_prefix(b"=") {
                let s = &s[s.iter().take_while(|&&c| is_space(c)).count()..];
                if let Some(s) = s.strip_prefix(b"\"")
                    && let Some(end) = s.iter().position(|&c| c == b'"')
                {
                    return Some(s[..end].to_vec());
                }
            }
        }
        match body[search..].iter().position(|&c| c == b'\n') {
            Some(off) => search += off + 1,
            None => break,
        }
    }
    None
}

/// `module_relative(path, root)`.
fn module_relative(path: &str, root: &str) -> String {
    let rel = graph::relative_path(path, root);
    if rel == "lib.rs" || rel == "mod.rs" {
        return String::new();
    }
    let rel = rel.strip_suffix("/mod.rs").unwrap_or(&rel);
    let rel = rel.strip_suffix(".rs").unwrap_or(rel);
    rel.replace('/', ".")
}

/// `resolve_mod(path, name, by_path)`.
fn resolve_mod(
    path: &str,
    name: &[u8],
    by_path: &HashMap<String, &crate::files::SourceFile>,
) -> Option<String> {
    let base = graph::file_base(path)?;
    let mut dir = parent(path)?;
    if base != "lib.rs" && base != "main.rs" && base != "mod.rs" {
        let stem = base.strip_suffix(".rs").unwrap_or(&base);
        dir = format!("{dir}/{stem}");
    }
    let name = text::text(name);
    [format!("{dir}/{name}.rs"), format!("{dir}/{name}/mod.rs")]
        .into_iter()
        .find(|p| by_path.contains_key(p))
}

impl Scanner for RustScanner {
    fn accepts(&self, relative: &str, _opts: &Value) -> bool {
        relative.ends_with(".rs") || relative.ends_with("/Cargo.toml") || relative == "Cargo.toml"
    }

    fn metadata_paths(&self, root: &str) -> Vec<String> {
        metadata_paths(root)
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
            _ => "rust".to_owned(),
        };
        let ns_prefix = match get(opts, "ns-prefix") {
            Some(Value::String(s)) => s.clone(),
            _ => prefix.clone(),
        };
        let by_path: HashMap<String, &crate::files::SourceFile> =
            files.iter().map(|f| (f.path.clone(), f)).collect();
        let mut surfaces: HashMap<String, Value> = HashMap::new();
        let surface = |path: &str,
                       surfaces: &mut HashMap<String, Value>,
                       by_path: &HashMap<String, &crate::files::SourceFile>| {
            surfaces
                .entry(path.to_owned())
                .or_insert_with(|| read_module(&by_path[path].source))
                .clone()
        };
        let mut queue: Vec<String> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        let mut reachable: Vec<String> = Vec::new();
        for n in ["lib.rs", "main.rs"] {
            let p = format!("{root}/{n}");
            if by_path.contains_key(&p) {
                queue.push(p);
            }
        }
        let mut at = 0;
        while at < queue.len() {
            let path = queue[at].clone();
            at += 1;
            if seen.insert(path.clone()) {
                reachable.push(path.clone());
                for n in seq(get(&surface(&path, &mut surfaces, &by_path), "mods")) {
                    if let Value::String(n) = n
                        && let Some(child) = resolve_mod(&path, n.as_bytes(), &by_path)
                        && !seen.contains(&child)
                    {
                        queue.push(child.to_owned());
                    }
                }
            }
        }
        reachable.sort();
        let mut metas = Vec::new();
        let mut by_rel: Vec<(String, Value)> = Vec::new();
        let mut by_file: HashMap<&str, usize> = HashMap::new();
        let mut lib_id: Option<Value> = None;
        let name = lib_name(root, &by_path);
        for path in &reachable {
            let rel = module_relative(path, root);
            let ns = graph::ns_join(&ns_prefix, &rel);
            let id = graph::id_of(&ns, Some(&Value::String(prefix.clone())));
            by_file.insert(path.as_str(), metas.len());
            if !by_rel.iter().any(|(r, _)| *r == rel) {
                by_rel.push((rel.clone(), id.clone()));
            }
            if rel.is_empty() && lib_id.is_none() {
                lib_id = Some(id.clone());
            }
            metas.push((path.clone(), rel, ns, id));
        }
        let suffix = |s: &str| -> Option<Value> {
            let s = s.trim_start_matches('.').trim_end_matches('.');
            if s.is_empty() {
                return None;
            }
            by_rel
                .iter()
                .find(|(r, _)| r == s)
                .map(|(_, id)| id.clone())
                .or_else(|| {
                    s.rsplit_once('.').and_then(|(parent, _)| {
                        by_rel
                            .iter()
                            .find(|(r, _)| r == parent)
                            .map(|(_, id)| id.clone())
                    })
                })
        };
        let resolve = |segs: &[Vec<u8>], m: &(String, String, String, Value)| -> Option<Value> {
            let head = segs.first()?.clone();
            let tail: Vec<String> = segs[1..].iter().map(|s| text::text(s)).collect();
            let tail = tail.join(".");
            let relative = &m.1;
            match head.as_slice() {
                b"crate" => suffix(&tail),
                b"self" => suffix(&graph::ns_join(relative, &tail)),
                b"super" => suffix(&graph::ns_join(
                    relative.rsplit_once('.').map(|(p, _)| p).unwrap_or(""),
                    &tail,
                )),
                _ => {
                    if Some(&head) == name.as_ref() {
                        suffix(&tail).or_else(|| lib_id.clone())
                    } else {
                        None
                    }
                }
            }
        };
        let mut classes = Vec::new();
        let mut edges: Vec<Value> = Vec::new();
        let mut foreigns: Vec<Value> = Vec::new();
        let mut commands_map: Vec<(Value, Value)> = Vec::new();
        let project_ids: HashSet<String> = metas
            .iter()
            .map(|m| crate::value::encode_key(&m.3))
            .collect();
        for m in &metas {
            let s = surface(&m.0, &mut surfaces, &by_path);
            let ops: Vec<Value> = seq(get(&s, "ops"))
                .iter()
                .map(|n| record(vec![("name", n.clone()), ("text", n.clone())]))
                .collect();
            let mut c = record(vec![
                ("id", m.3.clone()),
                ("ns", Value::String(m.2.clone())),
                (
                    "name",
                    Value::String(graph::file_base(&m.0).unwrap_or_default()),
                ),
                ("lang", Value::keyword("rust")),
                ("file", Value::String(graph::relative_path(&m.0, cwd))),
            ]);
            if !ops.is_empty() {
                edn::record::put(&mut c, "ops", Value::Vector(ops));
            }
            classes.push(c);
            let mut requires: Vec<Value> = Vec::new();
            let mut externs: Vec<Value> = Vec::new();
            let mut impls: Vec<Value> = Vec::new();
            for n in seq(get(&s, "mods")) {
                if let Value::String(n) = n
                    && let Some(path) = resolve_mod(&m.0, n.as_bytes(), &by_path)
                {
                    let id = metas[by_file[path.as_str()]].3.clone();
                    if !requires.contains(&id) {
                        requires.push(id);
                    }
                }
            }
            for u in seq(get(&s, "uses")) {
                if let Value::String(u) = u {
                    let segs = paths(u.as_bytes());
                    if let Some(target) = resolve(&segs, m)
                        && !requires.contains(&target)
                    {
                        requires.push(target);
                    }
                    if let Some(head) = segs.first() {
                        let head_text = text::text(head);
                        if head_text != "crate"
                            && head_text != "self"
                            && head_text != "super"
                            && Some(head) != name.as_ref()
                        {
                            let foreign = Value::keyword(&head_text);
                            if !externs.contains(&foreign) {
                                externs.push(foreign.clone());
                            }
                            if !foreigns.contains(&foreign) {
                                foreigns.push(foreign);
                            }
                        }
                    }
                }
            }
            if let (Some(lib_id), Some(name)) = (&lib_id, &name) {
                let (text_masked, _) = mask(&by_path[&m.0].source);
                let needle = {
                    let mut n = name.clone();
                    n.extend_from_slice(b"::");
                    n
                };
                let mut pos = 0;
                while let Some(off) =
                    find_subslice(&text_masked[pos.min(text_masked.len())..], &needle)
                {
                    let a = pos + off; // 0-based match start
                    if a == 0 || !crate::text::is_word(text_masked[a - 1]) {
                        if !requires.contains(lib_id) {
                            requires.push(lib_id.clone());
                        }
                        break;
                    }
                    pos = a + needle.len();
                }
            }
            for tr in seq(get(&s, "impls")) {
                if let Value::String(tr) = tr {
                    let bound = get(&s, "bindings")
                        .and_then(|b| b.get(&Value::String(tr.clone())))
                        .and_then(|b| match b {
                            Value::Vector(items) => Some(
                                items
                                    .iter()
                                    .map(|v| match v {
                                        Value::String(s) => s.clone().into_bytes(),
                                        _ => Vec::new(),
                                    })
                                    .collect::<Vec<Vec<u8>>>(),
                            ),
                            _ => None,
                        });
                    let segs = paths(tr.as_bytes());
                    let target = bound
                        .and_then(|segs| resolve(&segs, m))
                        .or_else(|| resolve(&segs, m))
                        .or_else(|| {
                            segs.first().and_then(|first| {
                                let first = text::text(first);
                                by_rel
                                    .iter()
                                    .find(|(r, _)| *r == first)
                                    .map(|(_, id)| id.clone())
                            })
                        });
                    if let Some(target) = target
                        && !impls.contains(&target)
                    {
                        impls.push(target);
                    }
                }
            }
            for (group, kind) in [
                (&requires, "dependency"),
                (&externs, "dependency"),
                (&impls, "implements"),
            ] {
                for to in group {
                    if m.3 != *to {
                        let e = record(vec![
                            ("from", m.3.clone()),
                            ("to", to.clone()),
                            ("kind", Value::keyword(kind)),
                        ]);
                        if !edges.contains(&e) {
                            edges.push(e);
                        }
                    }
                }
            }
            let cmds = seq(get(&s, "commands"));
            if !cmds.is_empty() {
                commands_map.push((m.3.clone(), Value::Vector(cmds.to_vec())));
            }
        }
        for id in foreigns {
            if !project_ids.contains(&crate::value::encode_key(&id)) {
                classes.push(graph::foreign_class(&id));
            }
        }
        Ok(record(vec![
            ("classes", Value::Vector(classes)),
            ("edges", Value::Vector(edges)),
            ("commands", Value::Map(commands_map)),
        ]))
    }
}
