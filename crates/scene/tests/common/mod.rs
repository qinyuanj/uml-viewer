//! Shared helpers for the geometry golden tests (`golden_geometry.rs`,
//! `route_goldens.rs`): fixture paths, vector access and the recursive
//! structural comparison ported from `edges_spec.close`.
#![allow(dead_code)] // each test crate uses a different subset

use edn::{Value, parse};
use scene::geom::{self, get};
use std::path::{Path, PathBuf};

/// Absolute tolerance of every golden number; goldens record it as
/// `:absolute-tolerance` and the comparison never loosens it.
pub const TOLERANCE: f64 = 1e-6;

pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `fixtures/<dir>/<name>` in the repository.
pub fn fixture(dir: &str, name: &str) -> PathBuf {
    repo().join("fixtures").join(dir).join(name)
}

pub fn read_edn(path: &Path) -> Value {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    parse(&text).unwrap()
}

/// The items of an EDN vector; anything else is empty.
pub fn items(v: Option<&Value>) -> Vec<Value> {
    match v {
        Some(Value::Vector(v)) => v.clone(),
        _ => vec![],
    }
}

/// The golden declares exactly the tolerance the comparison applies.
pub fn assert_golden_tolerance(golden: &Value, label: &str) {
    assert_eq!(
        Some(&Value::Float(TOLERANCE)),
        get(golden, "absolute-tolerance"),
        "{label}: :absolute-tolerance"
    );
}

/// Recursive structural comparison: maps must have the same keys (a missing
/// key fails by name), vectors the same length, scalars must be equal, and
/// every number must be finite and within [`TOLERANCE`].
pub fn assert_close(expected: &Value, actual: Option<&Value>, path: &str) {
    match expected {
        Value::Integer(_) | Value::Float(_) => {
            let a = geom::num(expected).unwrap();
            let Some(b) = actual.and_then(geom::num) else {
                panic!("{path}: expected {a}, actual missing or non-numeric");
            };
            assert!(b.is_finite(), "{path}: actual not finite ({b})");
            assert!(
                (a - b).abs() <= TOLERANCE,
                "{path}: expected {a} actual {b} (delta {})",
                (a - b).abs()
            );
        }
        Value::Map(entries) => {
            let Some(Value::Map(other)) = actual else {
                panic!("{path}: expected a map, got {actual:?}");
            };
            assert_eq!(entries.len(), other.len(), "{path}: map size");
            let actual = actual.unwrap();
            for (k, v) in entries {
                assert!(
                    other.iter().any(|(k2, _)| k2 == k),
                    "{path}: missing key {k:?}"
                );
                assert_close(v, actual.get(k), &format!("{path}{k:?}"));
            }
        }
        Value::Vector(items) => {
            let Some(Value::Vector(other)) = actual else {
                panic!("{path}: expected a vector, got {actual:?}");
            };
            assert_eq!(items.len(), other.len(), "{path}: vector length");
            for (i, v) in items.iter().enumerate() {
                assert_close(v, Some(&other[i]), &format!("{path}[{i}]"));
            }
        }
        _ => assert_eq!(Some(expected), actual, "{path}: scalar"),
    }
}
