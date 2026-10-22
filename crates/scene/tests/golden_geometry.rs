//! Golden geometry parity (Seam A): the `scene` projection of each JVM
//! golden view must match `fixtures/jvm-goldens/*.geometry.edn` exactly in
//! structure, with every finite number within 1e-6; NaN/Inf fail.

use edn::{Value, parse};
use scene::geom::{self, get};
use std::path::{Path, PathBuf};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(path: &Path) -> Value {
    parse(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Recursive structural comparison (port of `edges_spec.close`).
fn close(expected: &Value, actual: Option<&Value>, path: &str) {
    match expected {
        Value::Integer(_) | Value::Float(_) => {
            let a = geom::num(expected).unwrap();
            let Some(b) = actual.and_then(geom::num) else {
                panic!("{path}: expected {a}, actual missing or non-numeric");
            };
            assert!(b.is_finite(), "{path}: actual not finite ({b})");
            assert!(
                (a - b).abs() <= 1e-6,
                "{path}: expected {a} actual {b} (delta {})",
                (a - b).abs()
            );
        }
        Value::Map(entries) => {
            let Some(Value::Map(other)) = actual else {
                panic!("{path}: expected a map, got {actual:?}");
            };
            assert_eq!(entries.len(), other.len(), "{path}: map size");
            for (k, v) in entries {
                assert!(
                    other.iter().any(|(k2, _)| k2 == k),
                    "{path}: missing key {k:?}"
                );
                close(
                    v,
                    Some(actual.unwrap().get(k).unwrap()),
                    &format!("{path}{k:?}"),
                );
            }
        }
        Value::Vector(items) => {
            let Some(Value::Vector(other)) = actual else {
                panic!("{path}: expected a vector, got {actual:?}");
            };
            assert_eq!(items.len(), other.len(), "{path}: vector length");
            for (i, v) in items.iter().enumerate() {
                close(v, Some(&other[i]), &format!("{path}[{i}]"));
            }
        }
        _ => assert_eq!(Some(expected), actual, "{path}: scalar"),
    }
}

fn focuses(golden: &Value) -> Vec<Value> {
    match get(golden, "views") {
        Some(Value::Vector(views)) => views
            .iter()
            .map(|v| get(v, "focus").unwrap().clone())
            .collect(),
        _ => vec![],
    }
}

#[test]
fn projection_matches_every_golden_view_package_class_and_size() {
    for lang in ["lua", "typescript", "rust", "python"] {
        let dir = repo().join("fixtures/jvm-goldens");
        let doc = scene::ir::read_document(
            &std::fs::read_to_string(dir.join(format!("{lang}.ir.edn"))).unwrap(),
        )
        .unwrap();
        let golden = read(&dir.join(format!("{lang}.geometry.edn")));
        for (view, focus) in match get(&golden, "views") {
            Some(Value::Vector(views)) => views
                .iter()
                .map(|v| (v.clone(), get(v, "focus").unwrap().clone()))
                .collect::<Vec<_>>(),
            _ => panic!("no views"),
        } {
            let projection = scene::cli::project(&doc, None, &focus, &Value::Map(vec![]));
            let label = format!("{lang} {focus:?}");
            // Packages: count and rects by index.
            let want_packages = get(&view, "packages").unwrap();
            let got_packages = get(&projection, "packages").unwrap();
            close(
                want_packages,
                Some(got_packages),
                &format!("{label} packages"),
            );
            // Size fields.
            for k in ["w", "h", "min-x", "min-y"] {
                close(
                    &get(get(&view, "size").unwrap(), k).unwrap().clone(),
                    Some(get(get(&projection, "size").unwrap(), k).unwrap()),
                    &format!("{label} size {k}"),
                );
            }
            // Classes: id equality and rects, in order.
            let want_classes = match get(&view, "classes").unwrap() {
                Value::Vector(v) => v.clone(),
                _ => panic!(),
            };
            let got_classes = match get(&projection, "classes").unwrap() {
                Value::Vector(v) => v.clone(),
                _ => panic!(),
            };
            assert_eq!(want_classes.len(), got_classes.len(), "{label} class count");
            for (i, (want, got)) in want_classes.iter().zip(&got_classes).enumerate() {
                assert_eq!(get(want, "id"), get(got, "id"), "{label} class[{i}] id");
                close(
                    get(want, "rect").unwrap(),
                    Some(get(got, "rect").unwrap()),
                    &format!("{label} class[{i}] rect"),
                );
            }
        }
    }
}

#[test]
fn projection_matches_every_golden_edge_and_constrained_curve() {
    for lang in ["lua", "typescript", "rust", "python"] {
        let dir = repo().join("fixtures/jvm-goldens");
        let doc = scene::ir::read_document(
            &std::fs::read_to_string(dir.join(format!("{lang}.ir.edn"))).unwrap(),
        )
        .unwrap();
        let golden = read(&dir.join(format!("{lang}.geometry.edn")));
        for view in match get(&golden, "views") {
            Some(Value::Vector(views)) => views.clone(),
            _ => panic!(),
        } {
            let focus = get(&view, "focus").unwrap().clone();
            let projection = scene::cli::project(&doc, None, &focus, &Value::Map(vec![]));
            let label = format!("{lang} {focus:?}");
            let want = match get(&view, "edges").unwrap() {
                Value::Vector(v) => v.clone(),
                _ => panic!(),
            };
            let got = match get(&projection, "edges").unwrap() {
                Value::Vector(v) => v.clone(),
                _ => panic!(),
            };
            assert_eq!(want.len(), got.len(), "{label} edge count");
            for (i, (want, got)) in want.iter().zip(&got).enumerate() {
                for k in [
                    "from",
                    "to",
                    "kind",
                    "points",
                    "tip",
                    "behind",
                    "draw-bounds",
                    "curve",
                ] {
                    close(
                        get(want, k).unwrap_or(&Value::Nil),
                        Some(get(got, k).unwrap_or(&Value::Nil)),
                        &format!("{label} edge[{i}] {k}"),
                    );
                }
            }
        }
    }
}

#[test]
fn every_golden_file_is_read_and_has_the_expected_view_count() {
    let dir = repo().join("fixtures/jvm-goldens");
    for (lang, views) in [("lua", 1), ("typescript", 2), ("rust", 3), ("python", 2)] {
        let golden = read(&dir.join(format!("{lang}.geometry.edn")));
        assert_eq!(views, focuses(&golden).len(), "{lang} view count");
        assert_eq!(
            Some(&Value::Float(1e-6)),
            get(&golden, "absolute-tolerance")
        );
    }
}
