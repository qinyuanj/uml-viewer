//! IR idioms over `edn::Value` mirroring the `uml.edn` usage of the IR
//! pipeline. Generic record reading (`get`, `truthy`, `num`, `seq`,
//! `local_name`, `put`, `remove`, `record`) lives in `edn::record`; this
//! module keeps only the IR-specific idioms built on it: truthy reads where
//! `false` falls back like a missing key, id construction, and map copies.

use edn::Value;
use edn::record::{get, local_name};

/// The entry only when it is Lua truthy: absent, `nil` and `false` all read
/// as `None`. This is the scanner `get(m, k, d)` and the `policy.lua`
/// `default(g(m, k), d)` idiom, where `false` falls back like a missing key
/// (`get_truthy(m, k).unwrap_or(d)`).
pub fn get_truthy<'a>(m: &'a Value, key: &str) -> Option<&'a Value> {
    get(m, key).filter(|v| edn::record::truthy(Some(v)))
}

/// `policy.lua` `name(x)` on a value that must be a namespace id: the
/// `edn::record::local_name`, panicking on anything else.
pub fn id_name(x: &Value) -> String {
    local_name(x).expect("expected namespace id").to_owned()
}

/// `id(x)`: the keyword of `id_name(x)`.
pub fn id(x: &Value) -> Value {
    Value::keyword(&id_name(x))
}

/// `ids(xs)`.
pub fn ids(xs: &[Value]) -> Vec<Value> {
    xs.iter().map(id).collect()
}

pub fn contains(xs: &[Value], x: &Value) -> bool {
    xs.iter().any(|v| v == x)
}

/// `E.encode` as a hash key; ids are keywords, so encoding cannot fail here.
pub fn encode_key(value: &Value) -> String {
    edn::encode(value).unwrap_or_default()
}

/// `copy(m)`: shallow map copy; a non-map copies to the empty map.
pub fn copy_map(m: &Value) -> Value {
    match m {
        Value::Map(entries) => Value::Map(entries.clone()),
        _ => Value::Map(Vec::new()),
    }
}
