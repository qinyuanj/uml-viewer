//! Routing behaviour — the edges_spec.lua route oracle.

use edn::parse;
use scene::geom::{self, Rect};
use scene::route::route;

fn points_of(scene: &edn::Value, edge: usize) -> Vec<[f64; 2]> {
    let edges = match geom::get(scene, "edges").unwrap() {
        edn::Value::Vector(v) => v,
        _ => panic!(),
    };
    let ps = match geom::get(&edges[edge], "points").unwrap() {
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
    let edges = match geom::get(&s, "edges").unwrap() {
        edn::Value::Vector(v) => v,
        _ => panic!(),
    };
    assert_eq!(1, edges.len());
    assert_eq!(
        Some(&edn::Value::keyword("triangle")),
        geom::get(&edges[0], "head")
    );
    assert_eq!(
        Some(&edn::Value::Bool(true)),
        geom::get(&edges[0], "dashed?")
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
        scene::route::Face::Left,
        scene::route::face(r, [-50.0, 50.0])
    );
    assert_eq!(
        scene::route::Face::Right,
        scene::route::face(r, [450.0, 50.0])
    );
    assert_eq!(
        scene::route::Face::Top,
        scene::route::face(r, [200.0, -30.0])
    );
    assert_eq!(
        scene::route::Face::Bottom,
        scene::route::face(r, [200.0, 150.0])
    );
}
