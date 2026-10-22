//! Byte-string helpers mirroring the Lua string semantics the scanners were
//! written against: 1-based inclusive `sub`, ASCII character classes and Lua
//! `%s` whitespace. Sources are raw bytes; captures become `String` only at
//! the EDN boundary.

/// Lua `string.sub`: 1-based, inclusive; negative counts from the end.
pub fn sub(s: &[u8], i: i64, j: i64) -> &[u8] {
    let n = s.len() as i64;
    let start = if i < 0 {
        (n + i + 1).max(1)
    } else {
        i.min(n + 1)
    };
    let finish = if j < 0 { (n + j + 1).max(0) } else { j.min(n) };
    if start > finish {
        &[]
    } else {
        &s[(start - 1) as usize..finish as usize]
    }
}

/// Lua `s:sub(i, i)` as a byte (1-based); `None` outside the string.
pub fn at(s: &[u8], i: usize) -> Option<u8> {
    if i == 0 { None } else { s.get(i - 1).copied() }
}

/// Lua `s:find(needle, i, true)`: 1-based start, plain bytes, inclusive start.
pub fn find_plain(s: &[u8], needle: &[u8], i: usize) -> Option<usize> {
    if needle.is_empty() {
        return Some(i.max(1).min(s.len() + 1));
    }
    let start = i.saturating_sub(1);
    s.get(start..)?
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| start + p + 1)
}

/// Lua `%s`: space, tab, newline, vertical tab, form feed, carriage return.
pub fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r')
}

/// Lua `[%w_]`: ASCII alphanumeric or underscore.
pub fn is_word(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// Lua `[%a_]`.
pub fn is_alpha_(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

/// TypeScript identifier byte class `[%w_$]`.
pub fn is_ts_ident(c: u8) -> bool {
    is_word(c) || c == b'$'
}

/// Captured bytes to a `String` for the EDN boundary. Scanner captures are
/// ASCII by construction; lossy conversion keeps exotic bytes from aborting
/// a scan the Lua version would have carried through.
pub fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// Trims Lua `%s` from both ends (`(s:gsub('^%s+',''):gsub('%s+$',''))`).
pub fn trim(s: &[u8]) -> &[u8] {
    let start = s.iter().position(|&c| !is_space(c)).unwrap_or(s.len());
    let end = s.iter().rposition(|&c| !is_space(c)).map_or(0, |p| p + 1);
    &s[start..end.max(start)]
}
