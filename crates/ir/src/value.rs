//! Helpers over `edn::Value` mirroring the `uml.edn` idioms the IR pipeline
//! uses: keyword lookup that treats EDN `nil` as missing, scanner-style
//! defaults that also skip `false`, and record construction.

use edn::Value;

pub fn kw(name: &str) -> Value {
    Value::Keyword(name.to_owned())
}

/// Builds a map from keyword-named entries (`uml.edn.from_table`).
/// Encoding sorts keys, so construction order is not observable.
pub fn record(entries: Vec<(&str, Value)>) -> Value {
    Value::Map(entries.into_iter().map(|(k, v)| (kw(k), v)).collect())
}

/// `uml.edn.get` + policy's `get`: map-only, EDN `nil` counts as missing.
pub fn get<'a>(m: &'a Value, key: &str) -> Option<&'a Value> {
    match m.get(&Value::Keyword(key.to_owned())) {
        Some(Value::Nil) | None => None,
        value => value,
    }
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

/// Lua truthiness over a `get` result: absent, EDN `nil` and `false` are false.
pub fn truthy(value: Option<&Value>) -> bool {
    !matches!(value, None | Some(Value::Nil) | Some(Value::Bool(false)))
}

/// Numeric comparison helper: integers and floats compare by value.
pub fn as_f64(value: &Value) -> Option<f64> {
    match value {
        Value::Integer(n) => Some(*n as f64),
        Value::Float(x) => Some(*x),
        _ => None,
    }
}

/// `seq(v)`: vectors pass through, anything else is empty.
pub fn seq(value: Option<&Value>) -> &[Value] {
    match value {
        Some(Value::Vector(items)) => items,
        _ => &[],
    }
}

/// `name(x)`: strings are themselves; keywords/symbols take the last `/` segment.
pub fn name(x: &Value) -> String {
    match x {
        Value::String(s) => s.clone(),
        Value::Keyword(n) | Value::Symbol(n) => n.rsplit('/').next().unwrap_or(n).to_owned(),
        _ => panic!("expected namespace id"),
    }
}

/// `id(x)`: the keyword of `name(x)`.
pub fn id(x: &Value) -> Value {
    kw(&name(x))
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

/// `copy(m)`: shallow map copy.
pub fn copy_map(m: &Value) -> Value {
    match m {
        Value::Map(entries) => Value::Map(entries.clone()),
        _ => Value::Map(Vec::new()),
    }
}

/// `put(m, k, v)`: assoc a keyword key, returning the map.
pub fn put(m: &Value, key: &str, value: Value) -> Value {
    let mut out = copy_map(m);
    out.assoc(kw(key), value);
    out
}

/// `drop(m, k)`: map without the keyword key.
pub fn drop_key(m: &Value, key: &str) -> Value {
    match m {
        Value::Map(entries) => Value::Map(
            entries
                .iter()
                .filter(|(k, _)| *k != kw(key))
                .cloned()
                .collect(),
        ),
        _ => Value::Map(Vec::new()),
    }
}
