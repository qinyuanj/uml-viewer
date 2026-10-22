//! Companion, recovery and ownership records are EDN maps with keyword keys,
//! both on disk and in memory (the Lua code converted between plain tables
//! and EDN with `to_edn`/`from_edn`; keeping the EDN form throughout removes
//! that indirection without changing the on-disk contract).

use edn::Value;

/// Map lookup by keyword name (`get(record, "runner-token")` reads
/// `:runner-token`). `None` for missing keys, explicit nil and non-maps.
pub fn get<'a>(record: &'a Value, key: &str) -> Option<&'a Value> {
    match record.get(&Value::keyword(key)) {
        Some(Value::Nil) | None => None,
        some => some,
    }
}

/// Insert or replace a keyword entry; no-op on non-maps.
pub fn put(record: &mut Value, key: &str, value: Value) {
    record.assoc(Value::keyword(key), value);
}

pub fn remove(record: &mut Value, key: &str) {
    if let Value::Map(entries) = record {
        entries.retain(|(k, _)| *k != Value::keyword(key));
    }
}

pub fn map(entries: Vec<(&str, Value)>) -> Value {
    Value::Map(
        entries
            .into_iter()
            .map(|(key, value)| (Value::keyword(key), value))
            .collect(),
    )
}

/// The textual name of a keyword, symbol or string (Lua `name()`).
pub fn name(value: &Value) -> Option<String> {
    match value {
        Value::Keyword(name) | Value::Symbol(name) | Value::String(name) => Some(name.clone()),
        _ => None,
    }
}

pub fn get_name(record: &Value, key: &str) -> Option<String> {
    get(record, key).and_then(name)
}

pub fn get_i64(record: &Value, key: &str) -> Option<i64> {
    match get(record, key) {
        Some(Value::Integer(n)) => Some(*n),
        Some(Value::Float(n)) if n.fract() == 0.0 => Some(*n as i64),
        _ => None,
    }
}

pub fn get_bool(record: &Value, key: &str) -> Option<bool> {
    match get(record, key) {
        Some(Value::Bool(b)) => Some(*b),
        _ => None,
    }
}

pub fn get_vec(record: &Value, key: &str) -> Option<Vec<Value>> {
    match get(record, key) {
        Some(Value::Vector(items)) => Some(items.clone()),
        _ => None,
    }
}

pub fn string_vec(value: Option<Vec<Value>>) -> Vec<String> {
    value
        .unwrap_or_default()
        .iter()
        .filter_map(|item| match item {
            Value::String(text) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

pub fn is_map(value: &Value) -> bool {
    matches!(value, Value::Map(_))
}

pub fn is_empty_map(value: &Value) -> bool {
    matches!(value, Value::Map(entries) if entries.is_empty())
}

/// Deep equality ignoring the cleanup checkpoint (Lua `ownership_changed`).
pub fn without_cleanup_phase(value: &Value) -> Value {
    let mut copy = value.clone();
    remove(&mut copy, "cleanup-phase");
    copy
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookups_see_keywords_strings_and_absent_nil() {
        let record = edn::parse(
            r#"{:backend :herdr :session "uml-viewer-A" :missing nil :pid 42 :flag true}"#,
        )
        .unwrap();
        assert_eq!(Some("herdr".to_owned()), get_name(&record, "backend"));
        assert_eq!(
            Some("uml-viewer-A".to_owned()),
            get_name(&record, "session")
        );
        assert_eq!(None, get(&record, "missing"));
        assert_eq!(Some(42), get_i64(&record, "pid"));
        assert_eq!(Some(true), get_bool(&record, "flag"));
    }

    #[test]
    fn put_replaces_and_appends_keyword_entries() {
        let mut record = map(vec![("session", Value::String("a".to_owned()))]);
        put(&mut record, "session", Value::String("b".to_owned()));
        put(&mut record, "pane", Value::String("w1:p1".to_owned()));
        assert_eq!(Some("b".to_owned()), get_name(&record, "session"));
        assert_eq!(Some("w1:p1".to_owned()), get_name(&record, "pane"));
        assert_eq!(
            2,
            match &record {
                Value::Map(entries) => entries.len(),
                _ => 0,
            }
        );
    }
}
