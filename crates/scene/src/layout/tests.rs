//! Text width and class box lines — the layout_text_spec.lua oracle plus the
//! member-line rules exercised by viewer_spec.

use crate::layout::{CHAR_W, class_box_size, class_lines, text_w};
use edn::parse;
use edn::record::{get, local_name};

#[test]
fn jvm_width_for_ascii_two_byte_and_astral_text() {
    assert_eq!(8.0, CHAR_W);
    assert_eq!(0.0, text_w(None));
    assert_eq!(40.0, text_w(Some("Hello")));
    assert_eq!(24.0, text_w(Some("«α»")));
    // U+1F600 is one UTF-16 surrogate pair on the JVM: two units.
    assert_eq!(16.0, text_w(Some("\u{1F600}")));
}

#[test]
fn full_width_cjk_counts_as_two_half_width_characters() {
    assert_eq!(32.0, text_w(Some("中文")));
    assert_eq!(16.0 + 8.0 * 3.0 + 16.0, text_w(Some("类Box提")));
    assert_eq!(16.0, text_w(Some("，"))); // U+FF0C full-width comma
    assert_eq!(16.0, text_w(Some("。"))); // U+3002 CJK punctuation
    assert_eq!(16.0, text_w(Some("한"))); // Hangul syllable
    assert_eq!(8.0, text_w(Some("—"))); // U+2014 stays half width
    assert_eq!(8.0, text_w(Some("ｱ"))); // U+FF71 half-width katakana
}

#[test]
fn chinese_names_widen_class_boxes() {
    let (w, _, _) = class_box_size(&parse("{:name \"中文类名中文类名中文类名\"}").unwrap());
    assert_eq!(24.0 + 12.0 * 16.0, w);
}

#[test]
fn member_lines_show_stereotype_public_members_and_rules() {
    let c = parse(
        "{:id :a :name \"a\" :stereotype :interface :fields [{:text \"field\"}] :ops [{:text \"visible\"} {:text \"secret\" :private true}]}",
    )
    .unwrap();
    let lines = class_lines(&c);
    let texts: Vec<String> = lines
        .iter()
        .filter_map(|l| match get(l, "text") {
            Some(edn::Value::String(s)) => Some(s.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(vec!["«interface»", "a", "field", "visible"], texts);
    let rules = lines
        .iter()
        .filter(|l| local_name(get(l, "kind").unwrap()).unwrap_or_default() == "rule")
        .count();
    assert_eq!(2, rules);
    let (_, h, _) = class_box_size(&c);
    // 24 base + name/stereo 2*18 + field/op 2*18 + rules 2*12.
    assert_eq!(24.0 + 4.0 * 18.0 + 2.0 * 12.0, h);
}

#[test]
fn hidden_members_and_children_replace_member_lines() {
    let c = parse("{:name \"a\" :contents [{:id :a.b :name \"B\" :drill? true}]}").unwrap();
    let lines = class_lines(&c);
    let kinds: Vec<String> = lines
        .iter()
        .map(|l| {
            local_name(get(l, "kind").unwrap())
                .unwrap_or_default()
                .to_owned()
        })
        .collect();
    assert_eq!(vec!["name", "rule", "child"], kinds);
    let child = &lines[2];
    assert_eq!(Some(&edn::Value::keyword("a.b")), get(child, "id"),);
    let hidden = parse("{:name \"a\" :hide-members true :fields [{:text \"f\"}]}").unwrap();
    assert_eq!(1, class_lines(&hidden).len());
}

#[test]
fn box_width_grows_with_the_widest_line() {
    // 24 + 10 half-width cells is below the 120 floor.
    let (w, _, _) = class_box_size(&parse("{:name \"abcdefghij\"}").unwrap());
    assert_eq!(120.0, w);
    let (w, _, _) = class_box_size(&parse("{:name \"abcdefghijklmnopqrstuvw\"}").unwrap());
    assert_eq!(24.0 + 23.0 * 8.0, w);
}

// Rank-internal span order (#183).

use crate::layout::{RankOrder, place};
use edn::Value;

fn placed(classes: &str, edges: &str, direction: &str, order: RankOrder) -> Vec<Value> {
    let classes = edn::record::seq(Some(&parse(classes).unwrap())).to_vec();
    let edges = edn::record::seq(Some(&parse(edges).unwrap())).to_vec();
    place(&classes, &edges, direction, order)
}

fn center(c: &Value, horizontal: bool) -> f64 {
    let r = crate::geom::Rect::from_value(get(c, "rect").unwrap()).unwrap();
    if horizontal { r.cy() } else { r.cx() }
}

/// Sum over every edge of the across distance between its two class centres.
fn span(cs: &[Value], edges: &str, horizontal: bool) -> f64 {
    let at = |id: &Value| {
        cs.iter()
            .find(|c| get(c, "id") == Some(id))
            .map(|c| center(c, horizontal))
            .unwrap()
    };
    edn::record::seq(Some(&parse(edges).unwrap()))
        .iter()
        .map(|e| (at(get(e, "from").unwrap()) - at(get(e, "to").unwrap())).abs())
        .sum()
}

fn ids(cs: &[Value]) -> Vec<String> {
    cs.iter()
        .map(|c| edn::encode(get(c, "id").unwrap()).unwrap())
        .collect()
}

const SPLIT: &str = "[{:id :a :name \"a\" :level 1} {:id :x :name \"x\" :level 1} \
                      {:id :b :name \"b\" :level 1}]";
const SPLIT_EDGES: &str =
    "[{:from :a :to :b :kind :dependency} {:from :b :to :a :kind :dependency}]";

#[test]
fn span_order_brings_together_a_dependent_pair_split_by_an_unrelated_class() {
    let base = placed(SPLIT, SPLIT_EDGES, "tb", RankOrder::Base);
    let span_order = placed(SPLIT, SPLIT_EDGES, "tb", RankOrder::Span);
    // The barycentric sweeps leave x between the pair.
    assert_eq!(":x", ids(&base)[1], "base order {:?}", ids(&base));
    assert!(span(&span_order, SPLIT_EDGES, false) < span(&base, SPLIT_EDGES, false));
    assert_ne!(
        ":x",
        ids(&span_order)[1],
        "span order {:?}",
        ids(&span_order)
    );
}

#[test]
fn span_order_only_moves_classes_within_their_rank() {
    let classes = "[{:id :s :name \"s\"} {:id :a :name \"a\"} {:id :x :name \"x\"} \
                   {:id :b :name \"b\"} {:id :t :name \"t\"}]";
    let edges = "[{:from :s :to :a :kind :dependency} {:from :s :to :x :kind :dependency} \
                 {:from :s :to :b :kind :dependency} {:from :a :to :t :kind :dependency} \
                 {:from :b :to :t :kind :dependency} {:from :x :to :a :kind :association}]";
    for direction in ["tb", "lr"] {
        let base = placed(classes, edges, direction, RankOrder::Base);
        let span_order = placed(classes, edges, direction, RankOrder::Span);
        let horizontal = direction == "lr";
        assert!(span(&span_order, edges, horizontal) <= span(&base, edges, horizontal) + 1e-9);
        for c in &base {
            let moved = span_order
                .iter()
                .find(|d| get(d, "id") == get(c, "id"))
                .unwrap();
            assert_eq!(get(c, "rank"), get(moved, "rank"), "{direction}: rank kept");
            let (a, b) = (
                crate::geom::Rect::from_value(get(c, "rect").unwrap()).unwrap(),
                crate::geom::Rect::from_value(get(moved, "rect").unwrap()).unwrap(),
            );
            assert_eq!((a.w, a.h), (b.w, b.h), "{direction}: box size kept");
            if horizontal {
                assert_eq!(a.x, b.x, "{direction}: along position kept");
            } else {
                assert_eq!(a.y, b.y, "{direction}: along position kept");
            }
        }
    }
}

#[test]
fn span_order_reduces_spans_for_horizontal_ranks_with_their_centring() {
    // lr ranks by dependency depth and centres each rank column; the ranks
    // here differ in height, so the centring offsets enter the spans.
    let classes = "[{:id :a :name \"a\"} {:id :b :name \"b\"} {:id :c :name \"c\"} \
                   {:id :d :name \"d\"} {:id :e :name \"e\"} {:id :f :name \"f\"}]";
    let edges = "[{:from :c :to :b :kind :dependency} {:from :f :to :d :kind :dependency} \
                 {:from :d :to :c :kind :dependency} {:from :f :to :c :kind :dependency} \
                 {:from :b :to :f :kind :dependency} {:from :f :to :a :kind :dependency}]";
    let base = placed(classes, edges, "lr", RankOrder::Base);
    let span_order = placed(classes, edges, "lr", RankOrder::Span);
    assert!(span(&span_order, edges, true) < span(&base, edges, true));
}

#[test]
fn horizontal_spans_are_measured_at_the_drawn_box_centres() {
    // Ports make in-space and out-space differ, so a box centre is not the
    // middle of its slot; the same improving case must still improve when
    // measured on the drawn rects.
    let port = |n: &str| format!("{{:id :{n}.dep :name \"{n}\"}}");
    let classes = format!(
        "[{{:id :a :name \"a\" :out-deps [{} {}]}} {{:id :b :name \"b\" :in-deps [{}]}} \
          {{:id :c :name \"c\" :out-deps [{}]}} {{:id :d :name \"d\"}} \
          {{:id :e :name \"e\" :in-deps [{} {}]}} {{:id :f :name \"f\"}}]",
        port("x"),
        port("y"),
        port("z"),
        port("w"),
        port("u"),
        port("v")
    );
    let edges = "[{:from :c :to :b :kind :dependency} {:from :f :to :d :kind :dependency} \
                 {:from :d :to :c :kind :dependency} {:from :f :to :c :kind :dependency} \
                 {:from :b :to :f :kind :dependency} {:from :f :to :a :kind :dependency}]";
    for direction in ["lr", "rl"] {
        let base = placed(&classes, edges, direction, RankOrder::Base);
        let span_order = placed(&classes, edges, direction, RankOrder::Span);
        assert!(
            span(&span_order, edges, true) < span(&base, edges, true),
            "{direction}"
        );
    }
}

#[test]
fn span_order_is_deterministic() {
    let classes = "[{:id :a :name \"a\" :level 1} {:id :x :name \"xxxxxxxxxxxxxxxxxxxxxxxx\" :level 1} \
                   {:id :b :name \"b\" :level 1} {:id :y :name \"y\" :level 1} \
                   {:id :c :name \"c\" :level 0} {:id :d :name \"d\" :level 0}]";
    let edges = "[{:from :a :to :b :kind :dependency} {:from :c :to :y :kind :dependency} \
                 {:from :d :to :a :kind :dependency} {:from :c :to :b :kind :dependency}]";
    let first = placed(classes, edges, "tb", RankOrder::Span);
    for _ in 0..5 {
        assert_eq!(first, placed(classes, edges, "tb", RankOrder::Span));
    }
}

#[test]
fn an_already_optimal_order_is_kept() {
    // One class per rank.
    let chain = "[{:id :a :name \"a\"} {:id :b :name \"b\"} {:id :c :name \"c\"}]";
    let chain_edges = "[{:from :a :to :b :kind :dependency} {:from :b :to :c :kind :dependency}]";
    assert_eq!(
        placed(chain, chain_edges, "tb", RankOrder::Base),
        placed(chain, chain_edges, "tb", RankOrder::Span)
    );
    // Parallel columns are already the least span; a swap would only tie.
    let columns =
        "[{:id :a :name \"a\"} {:id :b :name \"b\"} {:id :c :name \"c\"} {:id :d :name \"d\"}]";
    let column_edges = "[{:from :a :to :c :kind :dependency} {:from :b :to :d :kind :dependency}]";
    assert_eq!(
        placed(columns, column_edges, "tb", RankOrder::Base),
        placed(columns, column_edges, "tb", RankOrder::Span)
    );
    // A layered fan-in and fan-out, like the fs-monopoly root: the
    // barycentric sweeps already reach the least span.
    let layered = "[{:id :p :name \"p\"} {:id :q :name \"q\"} {:id :c :name \"c\"} \
                   {:id :d :name \"d\"} {:id :e :name \"e\"} {:id :f :name \"f\"}]";
    let layered_edges = "[{:from :p :to :c :kind :dependency} {:from :q :to :c :kind :dependency} \
                         {:from :c :to :d :kind :dependency} {:from :c :to :e :kind :dependency} \
                         {:from :d :to :f :kind :dependency} {:from :e :to :f :kind :dependency} \
                         {:from :p :to :f :kind :dependency}]";
    assert_eq!(
        placed(layered, layered_edges, "tb", RankOrder::Base),
        placed(layered, layered_edges, "tb", RankOrder::Span)
    );
    // Equal-cost orders keep the barycentric order: two unrelated classes.
    let free = "[{:id :a :name \"a\" :level 1} {:id :b :name \"b\" :level 1}]";
    assert_eq!(
        placed(free, "[]", "tb", RankOrder::Base),
        placed(free, "[]", "tb", RankOrder::Span)
    );
}
