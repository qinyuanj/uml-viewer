//! Rectangle and gap geometry, ported from the `edges_spec`/`viewer_spec`
//! gap and hit geometry examples (the Lua busted suite is the oracle).

use edn::{Value, parse};
use scene::geom::{self, Rect};

fn rects(text: &str) -> Vec<Rect> {
    let Value::Vector(items) = parse(text).unwrap() else {
        panic!("rects must be a vector")
    };
    items.iter().map(|r| Rect::from_value(r).unwrap()).collect()
}

fn points(text: &str) -> Vec<[f64; 2]> {
    let Value::Vector(items) = parse(text).unwrap() else {
        panic!("points must be a vector")
    };
    items.iter().map(|p| geom::vec2(p).unwrap()).collect()
}

fn show(strokes: &[Vec<[f64; 2]>]) -> Value {
    Value::Vector(
        strokes
            .iter()
            .map(|s| Value::Vector(s.iter().map(|p| geom::point(*p)).collect()))
            .collect(),
    )
}

#[test]
fn gap_polyline_clips_merged_obstacles_with_jvm_padding() {
    // edges_spec: merged wall rects 40..60 and 55..70, pad 5.
    let ps = points("[[0 0] [100 0]]");
    let rs = rects("[{:x 40 :y -10 :w 20 :h 20} {:x 55 :y -10 :w 15 :h 20}]");
    let got = show(&geom::gap_polyline(&ps, &rs, 5.0));
    let want = parse("[[[0 0] [35 0]] [[75 0] [100 0]]]").unwrap();
    assert_eq!(want, got);
}

#[test]
fn degenerate_segment_gaps_to_nothing() {
    assert!(geom::gap_segment([0.0, 0.0], [0.5, 0.0], &[], 0.0).is_empty());
}

#[test]
fn rect_algebra_matches_the_lua_helpers() {
    let r = Rect::from_value(&parse("{:x 10 :y 20 :w 30 :h 40}").unwrap()).unwrap();
    assert_eq!(40.0, r.right());
    assert_eq!(60.0, r.bottom());
    assert_eq!(25.0, r.cx());
    assert_eq!(40.0, r.cy());
    assert!(r.contains(10.0, 60.0));
    assert!(!r.contains(41.0, 20.0));
    assert_eq!(Rect::new(5.0, 15.0, 40.0, 50.0), r.inflate(5.0));
    let other = Rect::new(0.0, 0.0, 20.0, 25.0);
    assert!(r.overlaps(other));
    assert_eq!(
        Rect::new(0.0, 0.0, 40.0, 60.0),
        geom::union(&[Some(r), Some(other)]).unwrap()
    );
    assert_eq!(None, geom::union(&[]));
}
