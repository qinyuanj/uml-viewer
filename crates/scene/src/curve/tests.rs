//! Curve behaviour — the edges_spec.lua curve oracle.

use crate::curve::{
    basis_path, constrain_ends, end_tangent, flatten_path, path_bounds, round_corners,
};
use crate::geom::{self, Rect};
use edn::parse;

fn ps(text: &str) -> Vec<[f64; 2]> {
    let edn::Value::Vector(items) = parse(text).unwrap() else {
        panic!()
    };
    items.iter().map(|p| geom::vec2(p).unwrap()).collect()
}

fn close(expected: &edn::Value, actual: &edn::Value, path: &str) {
    use edn::Value;
    match expected {
        Value::Integer(_) | Value::Float(_) => {
            let a = edn::record::num(expected).unwrap();
            let b = edn::record::num(actual).unwrap_or(f64::NAN);
            assert!(b.is_finite(), "{path}: actual not finite");
            assert!((a - b).abs() <= 1e-6, "{path}: expected {a} actual {b}");
        }
        Value::Map(entries) => {
            let Value::Map(other) = actual else {
                panic!("{path}: expected map")
            };
            assert_eq!(entries.len(), other.len(), "{path}");
            for (k, v) in entries {
                close(v, actual.get(k).unwrap(), &format!("{path}{k:?}"));
            }
        }
        Value::Vector(items) => {
            let Value::Vector(other) = actual else {
                panic!("{path}: expected vector")
            };
            assert_eq!(items.len(), other.len(), "{path}");
            for (i, v) in items.iter().enumerate() {
                close(v, &other[i], &format!("{path}[{i}]"));
            }
        }
        _ => assert_eq!(expected, actual, "{path}"),
    }
}

#[test]
fn short_paths_and_empty_inputs() {
    close(
        &parse("{:start [0 0] :ops []}").unwrap(),
        &basis_path(&[]),
        "empty",
    );
    close(
        &parse("{:start [2 3] :ops []}").unwrap(),
        &basis_path(&parse("[[2 3]]").unwrap().as_vector().unwrap()),
        "single",
    );
}

trait AsVector {
    fn as_vector(&self) -> Option<Vec<edn::Value>>;
}

impl AsVector for edn::Value {
    fn as_vector(&self) -> Option<Vec<edn::Value>> {
        match self {
            edn::Value::Vector(items) => Some(items.clone()),
            _ => None,
        }
    }
}

#[test]
fn round_corners_insets_sharp_turns() {
    let got = round_corners(&ps("[[0 0] [0 80] [80 80]]"), 24.0);
    let want = ps("[[0 0] [0 56] [24 80] [80 80]]");
    assert_eq!(want, got);
}

#[test]
fn end_tangent_and_flatten_cover_the_full_path() {
    let p = basis_path(
        &ps("[[0 0] [40 0] [40 80] [120 80]]")
            .iter()
            .map(|p| geom::point(*p))
            .collect::<Vec<_>>(),
    );
    let t = end_tangent(&p);
    close(&parse("[120 80]").unwrap(), &t[1], "tip");
    let behind = geom::vec2(&t[0]).unwrap();
    let tip = geom::vec2(&t[1]).unwrap();
    assert!((tip[1] - behind[1]).abs() > 1.0);
    let samples = flatten_path(&p, 0.0625);
    assert_eq!([0.0, 0.0], samples[0]);
    assert_eq!([120.0, 80.0], *samples.last().unwrap());
    assert!(samples.len() > 4);
}

#[test]
fn path_bounds_use_the_control_hull() {
    let p =
        parse("{:start [0 0] :ops [{:op :cubic :c1 [-80 10] :c2 [-80 70] :p [0 80]}]}").unwrap();
    assert_eq!(Rect::new(-80.0, 0.0, 80.0, 80.0), path_bounds(&p).unwrap());
}

#[test]
fn constrain_ends_rotates_shallow_handles_onto_the_face_cone() {
    let r = Rect::new(0.0, 0.0, 100.0, 40.0);
    for (p, q) in [
        ([100.0, 20.0], [100.0, 80.0]),
        ([0.0, 20.0], [0.0, 80.0]),
        ([50.0, 0.0], [110.0, 0.0]),
        ([50.0, 40.0], [110.0, 40.0]),
    ] {
        let raw = parse(&format!(
            "{{:start [{p0} {p1}] :ops [{{:op :cubic :c1 [{q0} {q1}] :c2 [{q0} {q1}] :p [{p0} {p1}]}} {{:op :line :p [160 20]}}]}}",
            p0 = p[0],
            p1 = p[1],
            q0 = q[0],
            q1 = q[1]
        ))
        .unwrap();
        let constrained = constrain_ends(&raw, Some(r), None);
        let ops = edn::record::get(&constrained, "ops")
            .unwrap()
            .as_vector()
            .unwrap();
        let handle = geom::vec2(edn::record::get(&ops[0], "c1").unwrap()).unwrap();
        let (dx, dy) = (handle[0] - p[0], handle[1] - p[1]);
        assert!((dx.abs() - dy.abs()).abs() < 1e-6, "{p:?} {q:?}");
        assert!((dx * dx + dy * dy - 3600.0).abs() < 1e-6, "{p:?} {q:?}");
        // The input path is not mutated.
        let raw_ops = edn::record::get(&raw, "ops").unwrap().as_vector().unwrap();
        assert_eq!(
            geom::vec2(edn::record::get(&raw_ops[0], "c1").unwrap()),
            Some(q)
        );
    }
}

#[test]
fn constrain_ends_pins_the_final_tangent() {
    let path = basis_path(
        &ps("[[0 0] [10 0] [10 400] [40 400]]")
            .iter()
            .map(|p| geom::point(*p))
            .collect::<Vec<_>>(),
    );
    let t = end_tangent(&constrain_ends(
        &path,
        None,
        Some(Rect::new(40.0, 360.0, 80.0, 80.0)),
    ));
    let behind = geom::vec2(&t[0]).unwrap();
    let tip = geom::vec2(&t[1]).unwrap();
    assert!((tip[0] - behind[0]).abs() + 1e-6 >= (tip[1] - behind[1]).abs());
    close(&parse("[40 400]").unwrap(), &t[1], "tip");
}
