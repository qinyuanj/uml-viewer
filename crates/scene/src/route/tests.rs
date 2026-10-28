//! Routing behaviour — the edges_spec.lua route oracle.

use crate::geom::{self, Rect};
use crate::route::route;
use edn::parse;

fn points_of(scene: &edn::Value, edge: usize) -> Vec<[f64; 2]> {
    let edges = match edn::record::get(scene, "edges").unwrap() {
        edn::Value::Vector(v) => v,
        _ => panic!(),
    };
    let ps = match edn::record::get(&edges[edge], "points").unwrap() {
        edn::Value::Vector(v) => v,
        _ => panic!(),
    };
    ps.iter().map(|p| geom::vec2(p).unwrap()).collect()
}

fn close_points(want: &str, got: &[[f64; 2]]) {
    let edn::Value::Vector(items) = parse(want).unwrap() else {
        panic!()
    };
    let want: Vec<[f64; 2]> = items.iter().map(|p| geom::vec2(p).unwrap()).collect();
    assert_eq!(want.len(), got.len(), "point count: want {want:?}");
    for (w, g) in want.iter().zip(got) {
        assert!(
            (w[0] - g[0]).abs() <= 1e-6 && (w[1] - g[1]).abs() <= 1e-6,
            "want {want:?} got {got:?}"
        );
    }
}

#[test]
fn drops_unlaid_endpoints_and_preserves_metadata_and_head_kinds() {
    let mut s = parse(
        "{:diagram {:edges [{:from :a :to :missing} {:from :a :to :b :kind :implements :dashed? true}]} :classes [{:id :a :rect {:x 0 :y 0 :w 40 :h 20}} {:id :b :rect {:x 0 :y 80 :w 40 :h 20}} {:id :missing}]}",
    )
    .unwrap();
    route(&mut s);
    let edges = match edn::record::get(&s, "edges").unwrap() {
        edn::Value::Vector(v) => v,
        _ => panic!(),
    };
    assert_eq!(1, edges.len());
    assert_eq!(
        Some(&edn::Value::keyword("triangle")),
        edn::record::get(&edges[0], "head")
    );
    assert_eq!(
        Some(&edn::Value::Bool(true)),
        edn::record::get(&edges[0], "dashed?")
    );
}

#[test]
fn uses_the_last_equally_smooth_clear_detour_and_first_blocked_fallback() {
    let mut s = parse(
        "{:diagram {:edges [{:from :a :to :b}]} :classes [{:id :a :rect {:x 0 :y 0 :w 40 :h 20}} {:id :b :rect {:x 80 :y 0 :w 40 :h 20}} {:id :wall :rect {:x 50 :y 0 :w 20 :h 20}}]}",
    )
    .unwrap();
    route(&mut s);
    close_points("[[20 20] [20 40] [100 40] [100 20]]", &points_of(&s, 0));
    // A giant wall blocks every detour: the first candidate is the fallback.
    let mut s = parse(
        "{:diagram {:edges [{:from :a :to :b}]} :classes [{:id :a :rect {:x 0 :y 0 :w 40 :h 20}} {:id :b :rect {:x 80 :y 0 :w 40 :h 20}} {:id :wall :rect {:x -100 :y -100 :w 400 :h 400}}]}",
    )
    .unwrap();
    route(&mut s);
    close_points("[[40 10] [80 10]]", &points_of(&s, 0));
}

#[test]
fn same_rank_edges_route_below_the_rank_box() {
    // Two side-by-side classes, one edge: straight drop between them.
    let mut s = parse(
        "{:diagram {:edges [{:from :a :to :b :kind :dependency}]} :classes [{:id :a :rect {:x 0 :y 0 :w 40 :h 20} :rank 0} {:id :b :rect {:x 80 :y 0 :w 40 :h 20} :rank 0}]}",
    )
    .unwrap();
    route(&mut s);
    let pts = points_of(&s, 0);
    assert!(pts.len() >= 2);
    for p in &pts {
        assert!(p[1] >= -1e-9, "stays at or below the boxes: {pts:?}");
    }
}

#[test]
fn face_picks_the_nearest_rect_side() {
    let r = Rect::new(0.0, 0.0, 400.0, 100.0);
    assert_eq!(
        crate::route::Face::Left,
        crate::route::face(r, [-50.0, 50.0])
    );
    assert_eq!(
        crate::route::Face::Right,
        crate::route::face(r, [450.0, 50.0])
    );
    assert_eq!(
        crate::route::Face::Top,
        crate::route::face(r, [200.0, -30.0])
    );
    assert_eq!(
        crate::route::Face::Bottom,
        crate::route::face(r, [200.0, 150.0])
    );
}

fn crosses(p: [f64; 2], q: [f64; 2], r: Rect) -> bool {
    // Liang-Barsky clip of segment p-q against the open rect.
    let (dx, dy) = (q[0] - p[0], q[1] - p[1]);
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    for (pp, qq) in [
        (-dx, p[0] - r.x),
        (dx, r.right() - p[0]),
        (-dy, p[1] - r.y),
        (dy, r.bottom() - p[1]),
    ] {
        if pp == 0.0 {
            if qq <= 0.0 {
                return false;
            }
        } else if pp < 0.0 {
            t0 = t0.max(qq / pp);
        } else {
            t1 = t1.min(qq / pp);
        }
        if t0 >= t1 {
            return false;
        }
    }
    true
}

#[test]
fn a_long_edge_passes_intermediate_ranks_beside_their_boxes() {
    // One class per level stacks a column; a -> d spans two ranks between.
    let d = parse(
        "{:packages [{:id :p :classes [{:id :a :name \"a\" :level 3} {:id :b :name \"b\" :level 2} {:id :c :name \"c\" :level 1} {:id :d :name \"d\" :level 0}]}] :edges [{:from :a :to :b} {:from :b :to :c} {:from :c :to :d} {:from :a :to :d}]}",
    )
    .unwrap();
    let mut s = crate::layout::layout(&d);
    route(&mut s);
    assert!(
        edn::record::get(&s, "lanes").is_none(),
        "lanes stay internal"
    );
    let classes = edn::record::seq(edn::record::get(&s, "classes"));
    let rect = |id: &str| {
        let c = classes
            .iter()
            .find(|c| edn::record::get(c, "id") == Some(&edn::Value::keyword(id)))
            .unwrap();
        Rect::from_value(edn::record::get(c, "rect").unwrap()).unwrap()
    };
    let pts = points_of(&s, 3);
    for id in ["b", "c"] {
        for w in pts.windows(2) {
            assert!(
                !crosses(w[0], w[1], rect(id)),
                "a -> d crosses {id}: {pts:?}"
            );
        }
    }
}
