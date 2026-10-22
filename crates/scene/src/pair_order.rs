//! Clojure 1.12.0 `group-by` key enumeration order (port of
//! `love/uml/domain/pair_order.lua`). Only distinct directed keyword pairs
//! are accepted, in first encounter order. The hash chain is Murmur3 with
//! the exact Clojure constants; in Rust the mod-2^32 arithmetic is simply
//! wrapping u32.

use edn::Value;

fn mixk(k: u32) -> u32 {
    k.wrapping_mul(0xCC9E_2D51)
        .rotate_left(15)
        .wrapping_mul(0x1B87_3593)
}

fn mixh(h: u32, k: u32) -> u32 {
    (h ^ k)
        .rotate_left(13)
        .wrapping_mul(5)
        .wrapping_add(0xE654_6B64)
}

fn fmix(h: u32, n: u32) -> u32 {
    let mut h = h ^ n;
    h ^= h >> 16;
    h = h.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 13;
    h = h.wrapping_mul(0xC2B2_AE35);
    h ^ (h >> 16)
}

fn utf16(s: &str) -> Vec<u32> {
    let mut out = Vec::new();
    for unit in s.encode_utf16() {
        out.push(unit as u32);
    }
    out
}

fn string_hash(s: &str) -> u32 {
    let units = utf16(s);
    let mut h: u32 = 0;
    let mut i = 0;
    while i + 1 < units.len() {
        h = mixh(h, mixk(units[i] + units[i + 1] * 65536));
        i += 2;
    }
    if units.len() % 2 == 1 {
        h ^= mixk(units[units.len() - 1]);
    }
    fmix(h, units.len() as u32 * 2)
}

fn keyword_hash(k: &str) -> u32 {
    let (ns, name) = match k.split_once('/') {
        Some((ns, name)) => (Some(ns), name),
        None => (None, k),
    };
    let h = string_hash(name);
    let mut nh: u32 = 0;
    if let Some(ns) = ns {
        for c in utf16(ns) {
            nh = nh.wrapping_mul(31).wrapping_add(c);
        }
    }
    // Java's signed >> in Util.hashCombine, unlike Murmur's unsigned >>>.
    let signed = h as i32;
    let in_i64 = (nh as i64) + 2_654_435_769i64 + (h as i64) * 64 + ((signed >> 2) as i64);
    let inner = in_i64.rem_euclid(1i64 << 32) as u32;
    (h ^ inner).wrapping_add(2_654_435_769)
}

fn kw_name(v: &Value) -> &str {
    match v {
        Value::Keyword(n) => n.as_str(),
        _ => panic!("pair_order only hashes keyword pairs"),
    }
}

fn pair_hash(pair: &(Value, Value)) -> u32 {
    let h = 31u32.wrapping_add(keyword_hash(kw_name(&pair.0)));
    let h = h
        .wrapping_mul(31)
        .wrapping_add(keyword_hash(kw_name(&pair.1)));
    fmix(mixh(0, mixk(h)), 2)
}

/// The enumeration order of a Clojure map built by `group-by`: insertion
/// order up to 8 distinct keys, then HAMT slot order (low five-bit hash
/// fragments first, ties by first encounter).
pub fn indices(pairs: &[(Value, Value)]) -> Vec<usize> {
    let mut out: Vec<usize> = (0..pairs.len()).collect();
    if pairs.len() <= 8 {
        return out;
    }
    let hashes: Vec<u32> = pairs.iter().map(pair_hash).collect();
    out.sort_by(|&a, &b| {
        let (mut x, mut y) = (hashes[a], hashes[b]);
        for _ in 0..7 {
            if x % 32 != y % 32 {
                return (x % 32).cmp(&(y % 32));
            }
            x /= 32;
            y /= 32;
        }
        a.cmp(&b)
    });
    out
}
