//! Companion, recovery and ownership records are EDN maps with keyword keys,
//! both on disk and in memory (the Lua code converted between plain tables
//! and EDN with `to_edn`/`from_edn`; keeping the EDN form throughout removes
//! that indirection without changing the on-disk contract).
//!
//! Generic record reading lives in `edn::record`; this module keeps only
//! the record-specific readers built on it. Companion names are the full
//! text of a keyword (`companion.lua` `name`), hence `text`.

use edn::Value;
use edn::record::{get, text};

pub fn get_name(record: &Value, key: &str) -> Option<String> {
    get(record, key).and_then(text).map(str::to_owned)
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
    edn::record::remove(&mut copy, "cleanup-phase");
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
        assert_eq!(None, get_name(&record, "missing"));
        assert_eq!(Some(42), get_i64(&record, "pid"));
        assert_eq!(Some(true), get_bool(&record, "flag"));
    }
}
