//! #148 route regression (Seam A): a three-class document whose
//! `loan-service -> loan` edge the original viewer draws as one straight
//! segment. The expected geometry in `fixtures/route-regression/` is static
//! data from the pre-#144 Lua scene core; every number must match within
//! 1e-6, and the edges keep their strokes so a hidden or faded edge fails.

use edn::{Value, parse};
use scene::geom::{self, get};
use std::path::{Path, PathBuf};

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/route-regression")
        .join(name)
}

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
                close(v, actual.unwrap().get(k), &format!("{path}{k:?}"));
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

fn items(v: Option<&Value>) -> Vec<Value> {
    match v {
        Some(Value::Vector(v)) => v.clone(),
        _ => vec![],
    }
}

fn compiled() -> Value {
    let doc =
        scene::ir::read_document(&std::fs::read_to_string(fixture("route148.ir.edn")).unwrap())
            .unwrap();
    scene::document::compile_view(&doc, &[], &[], &Value::Map(vec![]))
}

fn expected() -> Value {
    parse(&std::fs::read_to_string(fixture("route148.geometry.edn")).unwrap()).unwrap()
}

#[test]
fn unobstructed_dependency_is_a_direct_segment_as_in_the_original() {
    let scene = compiled();
    let edge = items(get(&scene, "edges"))
        .into_iter()
        .find(|e| get(e, "to").map(geom::name).as_deref() == Some("d0-loan"))
        .expect("loan-service -> loan edge");
    assert_eq!(
        2,
        items(get(&edge, "points")).len(),
        "no right-angle detour"
    );
    assert_eq!(1, items(get(&edge, "strokes")).len(), "edge is drawn");
}

#[test]
fn class_boxes_size_and_edge_geometry_match_the_original_scene() {
    let scene = compiled();
    let want = expected();
    close(get(&want, "size").unwrap(), get(&scene, "size"), "size");
    let (want_classes, got_classes) = (items(get(&want, "classes")), items(get(&scene, "classes")));
    assert_eq!(want_classes.len(), got_classes.len(), "class count");
    for (i, (w, g)) in want_classes.iter().zip(&got_classes).enumerate() {
        assert_eq!(get(w, "id"), get(g, "id"), "class[{i}] id");
        close(
            get(w, "rect").unwrap(),
            get(g, "rect"),
            &format!("class[{i}] rect"),
        );
    }
    let (want_edges, got_edges) = (items(get(&want, "edges")), items(get(&scene, "edges")));
    assert_eq!(want_edges.len(), got_edges.len(), "edge count");
    let rect_of = |id: Option<&Value>| {
        got_classes
            .iter()
            .find(|c| get(c, "id") == id)
            .and_then(|c| get(c, "rect"))
            .and_then(geom::Rect::from_value)
    };
    for (i, (w, g)) in want_edges.iter().zip(&got_edges).enumerate() {
        let curve = scene::curve::constrain_ends(
            &scene::curve::basis_path(&items(get(g, "points"))),
            rect_of(get(g, "from")),
            rect_of(get(g, "to")),
        );
        for k in [
            "from",
            "to",
            "kind",
            "points",
            "tip",
            "behind",
            "draw-bounds",
            "strokes",
        ] {
            close(get(w, k).unwrap(), get(g, k), &format!("edge[{i}] {k}"));
        }
        close(
            get(w, "curve").unwrap(),
            Some(&curve),
            &format!("edge[{i}] curve"),
        );
    }
}

#[test]
fn repeated_compilation_gives_identical_geometry() {
    assert_eq!(compiled(), compiled());
}
