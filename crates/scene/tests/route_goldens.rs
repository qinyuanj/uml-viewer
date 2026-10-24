//! Route geometry goldens (#149, Seam A): representative routing scenarios
//! whose expected scene geometry in `fixtures/route-goldens/` is static data
//! produced once by the pre-#144 Lua/LOVE scene core. Every number must
//! match within 1e-6; edges are compared in order, with their strokes, so a
//! hidden, faded or reordered edge fails.
//!
//! | scenario      | what it pins                                                  |
//! |---------------|---------------------------------------------------------------|
//! | `direct`      | unobstructed connection is one straight segment               |
//! | `obstacle`    | a box blocks every direct/track/hop candidate: first clear    |
//! |               | non-reversing detour, including the original's end sync       |
//! | `tie`         | same-rank peers with a box between: equal-score detours, the  |
//! |               | last one wins; two edges in one hop group (`i` = 0, 1)        |
//! | `rank-hop`    | a connection crossing a rank goes through the gap between    |
//! |               | that rank's boxes                                             |
//! | `attachments` | several ends on one class face are spread along it           |
//! | `curve-lr`    | left-to-right: curved corners, end tangents, inheritance     |
//! |               | heads, three ends on one face                                 |

use edn::{Value, parse};
use scene::events::{self, Dims};
use scene::geom::{self, get};
use std::path::{Path, PathBuf};

const SCENARIOS: [&str; 6] = [
    "direct",
    "obstacle",
    "tie",
    "rank-hop",
    "attachments",
    "curve-lr",
];

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/route-goldens")
        .join(name)
}

fn document(scenario: &str) -> Value {
    let text = std::fs::read_to_string(fixture(&format!("{scenario}.ir.edn"))).unwrap();
    scene::ir::read_document(&text).unwrap()
}

fn compiled(scenario: &str) -> Value {
    scene::document::compile_view(&document(scenario), &[], &[], &Value::Map(vec![]))
}

fn expected(scenario: &str) -> Value {
    let text = std::fs::read_to_string(fixture(&format!("{scenario}.geometry.edn"))).unwrap();
    parse(&text).unwrap()
}

fn items(v: Option<&Value>) -> Vec<Value> {
    match v {
        Some(Value::Vector(v)) => v.clone(),
        _ => vec![],
    }
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

/// Size, class rects and every edge (in order) against the golden.
fn assert_matches_original(scenario: &str) {
    let scene = compiled(scenario);
    let want = expected(scenario);
    close(
        get(&want, "size").unwrap(),
        get(&scene, "size"),
        &format!("{scenario} size"),
    );
    let (want_classes, got_classes) = (items(get(&want, "classes")), items(get(&scene, "classes")));
    assert_eq!(
        want_classes.len(),
        got_classes.len(),
        "{scenario}: class count"
    );
    for (i, (w, g)) in want_classes.iter().zip(&got_classes).enumerate() {
        assert_eq!(get(w, "id"), get(g, "id"), "{scenario}: class[{i}] id");
        close(
            get(w, "rect").unwrap(),
            get(g, "rect"),
            &format!("{scenario} class[{i}] rect"),
        );
    }
    let (want_edges, got_edges) = (items(get(&want, "edges")), items(get(&scene, "edges")));
    assert_eq!(want_edges.len(), got_edges.len(), "{scenario}: edge count");
    let rect_of = |id: Option<&Value>| {
        got_classes
            .iter()
            .find(|c| get(c, "id") == id)
            .and_then(|c| get(c, "rect"))
            .and_then(geom::Rect::from_value)
    };
    for (i, (w, g)) in want_edges.iter().zip(&got_edges).enumerate() {
        let at = format!("{scenario} edge[{i}]");
        let curve = scene::curve::constrain_ends(
            &scene::curve::basis_path(&items(get(g, "points"))),
            rect_of(get(g, "from")),
            rect_of(get(g, "to")),
        );
        for k in [
            "from",
            "to",
            "kind",
            "head",
            "points",
            "tip",
            "behind",
            "draw-bounds",
            "strokes",
        ] {
            close(get(w, k).unwrap(), get(g, k), &format!("{at} {k}"));
        }
        close(
            get(w, "curve").unwrap(),
            Some(&curve),
            &format!("{at} curve"),
        );
    }
}

fn edge(scene: &Value, from: &str, to: &str) -> Value {
    items(get(scene, "edges"))
        .into_iter()
        .find(|e| {
            get(e, "from").map(geom::name).as_deref() == Some(from)
                && get(e, "to").map(geom::name).as_deref() == Some(to)
        })
        .unwrap_or_else(|| panic!("edge {from} -> {to}"))
}

fn rect(scene: &Value, id: &str) -> geom::Rect {
    items(get(scene, "classes"))
        .into_iter()
        .find(|c| get(c, "id").map(geom::name).as_deref() == Some(id))
        .and_then(|c| get(&c, "rect").and_then(geom::Rect::from_value))
        .unwrap_or_else(|| panic!("class {id}"))
}

fn points(e: &Value) -> Vec<[f64; 2]> {
    items(get(e, "points"))
        .iter()
        .map(|p| {
            let p = items(Some(p));
            [geom::num(&p[0]).unwrap(), geom::num(&p[1]).unwrap()]
        })
        .collect()
}

#[test]
fn unobstructed_direct_connection_matches_the_original() {
    assert_matches_original("direct");
    let scene = compiled("direct");
    let e = edge(&scene, "d0-order", "d0-line");
    assert_eq!(2, points(&e).len(), "one straight segment");
    assert_eq!(1, items(get(&e, "strokes")).len(), "edge is drawn");
}

#[test]
fn connection_blocked_by_one_class_box_matches_the_original() {
    assert_matches_original("obstacle");
    let scene = compiled("obstacle");
    let e = edge(&scene, "d0-source", "d0-sink");
    let filter = rect(&scene, "d0-filter");
    let ps = points(&e);
    assert!(ps.len() > 2, "detours instead of the straight line");
    assert!(
        ps.iter().any(|p| p[0] < filter.x),
        "leaves on the left of the blocking Filter box"
    );
}

#[test]
fn equal_score_candidates_pick_the_last_as_in_the_original() {
    assert_matches_original("tie");
    let scene = compiled("tie");
    let below = |from: &str, to: &str| {
        let r = rect(&scene, from);
        points(&edge(&scene, from, to))[1][1] - r.bottom()
    };
    // Three clear right-angle detours score alike; the last (the rank
    // track, 20 + 10i below the row) wins, not the first (10 + 10i).
    assert_eq!(20.0, below("d0-left", "d0-right"));
    assert_eq!(30.0, below("d0-right", "d0-left"));
}

#[test]
fn connection_hopping_a_rank_matches_the_original() {
    assert_matches_original("rank-hop");
    let scene = compiled("rank-hop");
    let ps = points(&edge(&scene, "d0-gateway", "d0-repo"));
    let (auth, limit) = (rect(&scene, "d0-auth"), rect(&scene, "d0-limit"));
    assert_eq!(4, ps.len(), "start, two hop points, end");
    let (top, bottom) = (auth.y.min(limit.y), auth.bottom().max(limit.bottom()));
    assert!(
        ps[1][1] < top,
        "first hop point {:?} above the middle rank",
        ps[1]
    );
    assert!(
        ps[2][1] > bottom,
        "second hop point {:?} below the middle rank",
        ps[2]
    );
}

#[test]
fn several_attachments_on_one_face_match_the_original() {
    assert_matches_original("attachments");
    let scene = compiled("attachments");
    let broker = rect(&scene, "d0-broker");
    let mut xs: Vec<f64> = ["d0-orders", "d0-billing", "d0-shipping"]
        .iter()
        .map(|to| points(&edge(&scene, "d0-broker", to))[0])
        .inspect(|p| assert_eq!(broker.bottom(), p[1], "leaves the bottom face"))
        .map(|p| p[0])
        .collect();
    xs.dedup();
    assert_eq!(3, xs.len(), "three distinct attachment points");
}

#[test]
fn curved_corners_and_end_tangents_left_to_right_match_the_original() {
    assert_matches_original("curve-lr");
    let scene = compiled("curve-lr");
    let e = edge(&scene, "d0-exporter", "d0-shape");
    let curve = scene::curve::basis_path(&items(get(&e, "points")));
    assert!(
        items(get(&curve, "ops"))
            .iter()
            .any(|op| get(op, "op").map(geom::name).as_deref() == Some("cubic")),
        "corners are curved"
    );
    let (tip, behind) = (point(get(&e, "tip")), point(get(&e, "behind")));
    let shape = rect(&scene, "d0-shape");
    assert_eq!(shape.right(), tip[0], "arrow tip on Shape's right face");
    assert!(
        behind[0] > tip[0],
        "arrives from the right, outside the box"
    );
}

fn point(v: Option<&Value>) -> [f64; 2] {
    let p = items(v);
    [geom::num(&p[0]).unwrap(), geom::num(&p[1]).unwrap()]
}

#[test]
fn repeated_compilation_gives_identical_geometry() {
    // Attachment, hop and endpoint groups live in per-call hash maps with
    // fresh seeds; every compilation must still produce the same scene.
    for scenario in SCENARIOS {
        let first = compiled(scenario);
        for _ in 0..16 {
            assert_eq!(first, compiled(scenario), "{scenario}");
        }
    }
}

#[test]
fn zoom_pan_and_window_size_leave_world_routes_unchanged() {
    let jvm = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/jvm-goldens/rust.ir.edn");
    let mut docs: Vec<Value> = SCENARIOS.iter().map(|s| document(s)).collect();
    docs.push(scene::ir::read_document(&std::fs::read_to_string(jvm).unwrap()).unwrap());
    for doc in docs {
        let start = scene::document::new_state(doc.clone(), None, None, None, &[], None);
        let routes = get(&start.scene, "edges").cloned();
        let mut s = start.clone();
        for (w, h) in [(1500.0, 920.0), (640.0, 480.0), (3840.0, 2160.0)] {
            let dims = Dims {
                window_w: w,
                window_h: h,
                ..Dims::default()
            };
            s = events::fit(&s, &dims);
            s = events::zoom_at(&s, 3.7, w / 3.0, h / 4.0);
            s = events::on_wheel(&s, 0.0, -4.0, &dims);
            s = events::on_wheel(&s, 2.0, 0.0, &dims);
            let ctrl = Dims {
                control: true,
                pointer: Some((10.0, 20.0)),
                ..dims.clone()
            };
            s = events::on_wheel(&s, 0.0, -3.0, &ctrl);
            assert_eq!(
                routes,
                get(&s.scene, "edges").cloned(),
                "camera changed routes"
            );
        }
        assert_ne!(
            (start.cam_x, start.cam_y, start.zoom),
            (s.cam_x, s.cam_y, s.zoom)
        );
        let fresh = scene::document::compile_view(&doc, &[], &[], &Value::Map(vec![]));
        assert_eq!(routes, get(&fresh, "edges").cloned(), "recompiled routes");
    }
}
