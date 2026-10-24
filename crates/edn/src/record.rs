//! Record reading: the single definition of the `edn.lua` reading rules
//! the scene core, IR, companion and viewer share (#151). Records are EDN
//! maps with keyword keys, addressed here by the keyword name
//! (`get(m, "runner-token")` reads `:runner-token`).

use crate::Value;

/// Map lookup by keyword name; absent keys, an explicit `nil` and non-maps
/// all read as `None` (Lua `g`).
pub fn get<'a>(m: &'a Value, key: &str) -> Option<&'a Value> {
    match m.get(&Value::keyword(key)) {
        Some(Value::Nil) | None => None,
        some => some,
    }
}

/// Map lookup that keeps an explicit `nil` (Lua `E.get` found-flag use), for
/// the few readers that tell "written as nil" from "not written".
pub fn get_raw<'a>(m: &'a Value, key: &str) -> Option<&'a Value> {
    m.get(&Value::keyword(key))
}

/// Lua truthiness of a flag entry: absent, `nil` and `false` are false;
/// every other value (`0`, `""`, `[]`, `1`, `:yes`) is true.
pub fn flag(m: &Value, key: &str) -> bool {
    truthy(get_raw(m, key))
}

/// [`flag`] over an already looked-up value.
pub fn truthy(v: Option<&Value>) -> bool {
    !matches!(v, None | Some(Value::Nil) | Some(Value::Bool(false)))
}

/// Integers and floats are both numbers (Lua has one number type).
pub fn num(v: &Value) -> Option<f64> {
    match v {
        Value::Integer(n) => Some(*n as f64),
        Value::Float(x) => Some(*x),
        _ => None,
    }
}

/// [`num`] of an optional value, `default` when absent or not a number.
pub fn num_or(v: Option<&Value>, default: f64) -> f64 {
    v.and_then(num).unwrap_or(default)
}

/// The id name the scene core and IR use (`geom.lua` / `policy.lua`
/// `name`): a string is itself; a keyword or symbol yields the segment after
/// its last `/`. `None` for every other value.
pub fn local_name(v: &Value) -> Option<&str> {
    match v {
        Value::String(s) => Some(s),
        Value::Keyword(n) | Value::Symbol(n) => n.rsplit('/').next(),
        _ => None,
    }
}

/// The full text of a keyword, symbol or string, namespace included
/// (`companion.lua` `name`). `None` for every other value. Deliberately
/// distinct from [`local_name`].
pub fn text(v: &Value) -> Option<&str> {
    match v {
        Value::Keyword(n) | Value::Symbol(n) | Value::String(n) => Some(n),
        _ => None,
    }
}

/// An optional vector as a slice; anything else is empty.
pub fn seq(v: Option<&Value>) -> &[Value] {
    match v {
        Some(Value::Vector(items)) => items,
        _ => &[],
    }
}

/// Set `key` to `value` in place, replacing the entry or appending it; an
/// explicit `nil` value is kept. No-op on non-maps.
pub fn put(m: &mut Value, key: &str, value: Value) {
    m.assoc(Value::keyword(key), value);
}

/// Drop the `key` entry in place. No-op on non-maps.
pub fn remove(m: &mut Value, key: &str) {
    if let Value::Map(entries) = m {
        let key = Value::keyword(key);
        entries.retain(|(k, _)| *k != key);
    }
}

/// A map from keyword-named entries. Encoding sorts keys, so construction
/// order is not observable in output.
pub fn record(entries: Vec<(&str, Value)>) -> Value {
    Value::Map(
        entries
            .into_iter()
            .map(|(key, value)| (Value::keyword(key), value))
            .collect(),
    )
}
