//! Scene composition — the edges_spec.lua compose oracle; golden geometry
//! parity is covered separately by golden_geometry.rs.

use edn::{Value, parse};
use scene::compose::{compile_diagram, compile_document};
use scene::geom::{self, Rect};

fn seq<'a>(v: &'a Value, key: &str) -> &'a [Value] {
    match geom::get(v, key) {
        Some(Value::Vector(items)) => items,
        _ => &[],
    }
}

#[test]
fn stacks_flat_diagrams_with_qualified_endpoints_and_prepared_strokes() {
    let s = compile_document(&parse(
        "{:title \"D\" :diagrams [{:packages [{:id :p :label \"P\" :classes [{:id :a} {:id :b}]}] :edges [{:from :a :to :b}]} {:packages [{:id :p :label \"P\" :classes [{:id :a} {:id :b}]}] :edges [{:from :a :to :b}]}]}",
    ).unwrap());
    assert_eq!(2, seq(&s, "sections").len());
    let edges = seq(&s, "edges");
    assert_eq!(2, edges.len());
    for (i, e) in edges.iter().enumerate() {
        let want_from = Value::keyword(&format!("d{i}-a"));
        let want_to = Value::keyword(&format!("d{i}-b"));
        assert_eq!(Some(&want_from), geom::get(e, "from"));
        assert_eq!(Some(&want_to), geom::get(e, "to"));
        assert!(!seq(e, "strokes").is_empty());
    }
    let tip_y = |e: &Value| geom::vec2(geom::get(e, "tip").unwrap()).unwrap()[1];
    assert!(tip_y(&edges[1]) > tip_y(&edges[0]));
    let h = geom::num(geom::get(geom::get(&s, "size").unwrap(), "h").unwrap()).unwrap();
    assert!(h > tip_y(&edges[1]));
}

#[test]
fn hide_edges_replaces_strokes_with_incoming_and_outgoing_indicators() {
    let s = compile_diagram(&parse(
        "{:hide-edges true :packages [{:id :p :label \"P\" :classes [{:id :a :name \"A\"} {:id :b :name \"B\"}]}] :edges [{:from :a :to :b :kind :dependency :violating true}]}",
    ).unwrap());
    let indicators = seq(&s, "dep-indicators");
    assert_eq!(2, indicators.len());
    for ind in indicators {
        assert_eq!(Some(&Value::Bool(true)), geom::get(ind, "violating"));
    }
}

#[test]
fn prepare_scene_clips_strokes_around_a_wall() {
    // The wall sits between the boxes; strokes split into two runs and the
    // hidden middle never hits.
    let s = compile_diagram(&parse(
        "{:packages [{:id :p :label \"P\" :classes [{:id :a} {:id :wall} {:id :b}]}] :edges [{:from :a :to :b :kind :dependency}]}",
    ).unwrap());
    let edges = seq(&s, "edges");
    assert_eq!(1, edges.len());
    let strokes = seq(&edges[0], "strokes");
    assert!(!strokes.is_empty());
    let wall = seq(&s, "classes")
        .iter()
        .find(|c| geom::get(c, "id") == Some(&Value::keyword("wall")))
        .unwrap();
    let wr = Rect::from_value(geom::get(wall, "rect").unwrap())
        .unwrap()
        .inflate(4.9);
    for stroke in strokes {
        let pts: Vec<[f64; 2]> = match stroke {
            Value::Vector(items) => items.iter().map(|p| geom::vec2(p).unwrap()).collect(),
            _ => panic!(),
        };
        for w in pts.windows(2) {
            let mid = [(w[0][0] + w[1][0]) / 2.0, (w[0][1] + w[1][1]) / 2.0];
            assert!(
                !wr.contains(mid[0], mid[1]),
                "stroke enters the wall: {mid:?}"
            );
        }
    }
}

#[test]
fn single_diagram_document_compiles_through_the_same_path() {
    let s = compile_document(
        &parse("{:title \"Solo\" :packages [{:id :p :label \"P\" :classes [{:id :a}]}] :edges []}")
            .unwrap(),
    );
    assert_eq!(1, seq(&s, "sections").len());
    assert_eq!(1, seq(&s, "classes").len());
    assert_eq!(
        Some(&Value::keyword("d0-a")),
        geom::get(&seq(&s, "classes")[0], "id")
    );
}
