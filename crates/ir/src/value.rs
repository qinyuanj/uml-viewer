//! IR idioms over `edn::Value` mirroring the `uml.edn` usage of the IR
//! pipeline. Generic record reading (`get`, `truthy`, `num`, `seq`,
//! `local_name`, `put`, `remove`, `record`) lives in `edn::record`; this
//! module keeps only the IR-specific idioms built on it: scanner-style
//! defaults that also skip `false`, id construction, and map copies.

use edn::Value;
use edn::record::{get, local_name};

pub fn kw(name: &str) -> Value {
    Value::Keyword(name.to_owned())
}

/// Scanner-style `get(m, k, d)`: `false` also counts as missing.
pub fn getd<'a>(m: &'a Value, key: &str) -> Option<&'a Value> {
    match get(m, key) {
        Some(Value::Bool(false)) => None,
        value => value,
    }
}

/// `default(v, d)`: `nil`, EDN `nil` and `false` fall back.
pub fn or<'a>(value: Option<&'a Value>, default: &'a Value) -> &'a Value {
    match value {
        None | Some(Value::Nil) | Some(Value::Bool(false)) => default,
        Some(value) => value,
    }
}

/// `policy.lua` `name(x)` on a value that must be a namespace id: the
/// `edn::record::local_name`, panicking on anything else.
pub fn id_name(x: &Value) -> String {
    local_name(x).expect("expected namespace id").to_owned()
}

/// `id(x)`: the keyword of `id_name(x)`.
pub fn id(x: &Value) -> Value {
    kw(&id_name(x))
}

/// `ids(xs)`.
pub fn ids(xs: &[Value]) -> Vec<Value> {
    xs.iter().map(id).collect()
}

pub fn contains(xs: &[Value], x: &Value) -> bool {
    xs.iter().any(|v| v == x)
}

/// `E.encode` as a hash key; ids are keywords, so encoding cannot fail here.
pub fn key_of(value: &Value) -> String {
    edn::encode(value).unwrap_or_default()
}

/// `copy(m)`: shallow map copy; a non-map copies to the empty map.
pub fn copy_map(m: &Value) -> Value {
    match m {
        Value::Map(entries) => Value::Map(entries.clone()),
        _ => Value::Map(Vec::new()),
    }
}
