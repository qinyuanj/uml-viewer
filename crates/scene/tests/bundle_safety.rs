//! Safe dependency bundles (#179) at the public scene compilation seam.
//! Fixed-geometry obstacle, crossing and endpoint cases live beside the
//! planner in `bundle.rs`; these cover real layouts and the final golden.

mod common;

use common::{assert_close, assert_golden_tolerance, items, read_edn};
use edn::record::{get, seq};
use edn::{Value, parse};

fn compile(text: &str) -> Value {
    let doc = scene::ir::read_document(text).unwrap();
    scene::document::compile_view(&doc, &[], &[], &parse("{}").unwrap())
}

fn member_counts(scene: &Value) -> Vec<usize> {
    seq(get(scene, "bundles"))
        .iter()
        .map(|b| seq(get(b, "members")).len())
        .collect()
}

fn unbundled(scene: &Value) -> bool {
    get(scene, "bundles").is_none()
        && seq(get(scene, "edges"))
            .iter()
            .all(|e| get(e, "branches").is_none())
}

fn fixture() -> Value {
    let path = common::fixture("bundle-goldens", "safe-channel.ir.edn");
    scene::ir::read_document(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn app_fan_out_final_geometry_matches_its_golden() {
    let projection = scene::cli::project(
        &fixture(),
        None,
        &parse("[]").unwrap(),
        &parse("{}").unwrap(),
    );
    let golden = read_edn(&common::fixture(
        "bundle-goldens",
        "safe-channel.geometry.edn",
    ));
    assert_golden_tolerance(&golden, "safe-channel");
    let actual = items(get(&projection, "edges"));
    assert_eq!(
        items(get(&golden, "edges")).len(),
        actual.len(),
        "logical edges kept"
    );
    for (i, expected) in items(get(&golden, "edges")).iter().enumerate() {
        for key in ["from", "to", "kind", "head", "deps", "tip"] {
            assert_close(
                get(expected, key).unwrap(),
                get(&actual[i], key),
                &format!("edge[{i}] {key}"),
            );
        }
        match get(expected, "branches") {
            Some(b) => assert_close(
                b,
                get(&actual[i], "branches"),
                &format!("edge[{i}] branches"),
            ),
            None => assert!(
                get(&actual[i], "branches").is_none(),
                "edge[{i}] keeps its base route"
            ),
        }
    }
    assert_close(
        get(&golden, "bundles").unwrap(),
        get(&projection, "bundles"),
        "final trunks",
    );
}

/// Only the two long parallel runs share: the third target's run overlaps
/// them too briefly, and the fourth edge is one diagonal that could only
/// join through a sideways jog.
#[test]
fn app_fan_out_shares_one_trunk_where_two_runs_stay_parallel() {
    let scene = scene::document::compile_view(&fixture(), &[], &[], &parse("{}").unwrap());
    assert_eq!(vec![2], member_counts(&scene));
    let targets: Vec<_> = seq(get(&scene, "edges"))
        .iter()
        .map(|e| get(e, "to").cloned())
        .collect();
    assert_eq!(
        vec![
            Some(Value::keyword("a")),
            Some(Value::keyword("b")),
            Some(Value::keyword("c")),
            Some(Value::keyword("d"))
        ],
        targets,
        "no dependency is re-attached to a representative endpoint"
    );
}

/// Two long lines converging on one box come within a channel pitch only
/// near it; they share that part and keep their own arrowheads. A one-rank
/// fan-in has no room for a trunk between its bends.
#[test]
fn foundation_fan_in_shares_only_the_converging_end() {
    let scene = compile(
        r#"{:hierarchical true :order [f c d k5 k4 k3 k2 k1 z e]
      :classes [{:id :d :level 6} {:id :c :level 6} {:id :f :level 6} {:id :k5 :level 5}
                {:id :k4 :level 4} {:id :k3 :level 3} {:id :k2 :level 2} {:id :k1 :level 1}
                {:id :z :level 0} {:id :e :level 0}]
      :edges [{:from :d :to :e :kind :dependency} {:from :c :to :e :kind :dependency}]}"#,
    );
    assert_eq!(vec![2], member_counts(&scene));
    let trunk = seq(get(&seq(get(&scene, "bundles"))[0], "strokes"));
    let ys: Vec<f64> = seq(Some(&trunk[0]))
        .iter()
        .map(|p| edn::record::num(&seq(Some(p))[1]).unwrap())
        .collect();
    assert!(
        ys.iter().all(|y| *y > 1000.0),
        "trunk near the target: {ys:?}"
    );
    for e in seq(get(&scene, "edges")) {
        assert_eq!(Some(&Value::keyword("e")), get(e, "to"));
    }
    let short = compile(
        r#"{:hierarchical true :order [app a b c]
      :classes [{:id :app :level 1} {:id :a :level 3} {:id :b :level 3} {:id :c :level 3}]
      :edges [{:from :a :to :app :kind :dependency} {:from :b :to :app :kind :dependency}
              {:from :c :to :app :kind :dependency}]}"#,
    );
    assert!(unbundled(&short), "no stub-length trunk");
}

#[test]
fn a_rank_hop_shares_unless_an_intervening_box_is_in_the_channel() {
    let hop = compile(
        r#"{:hierarchical true :order [f s m n a b]
      :classes [{:id :s :level 3} {:id :m :level 2} {:id :n :level 1} {:id :a :level 0}
                {:id :b :level 0} {:id :f.s :level 3}]
      :edges [{:from :s :to :a :kind :dependency} {:from :s :to :b :kind :dependency}]}"#,
    );
    assert_eq!(vec![2], member_counts(&hop));
    let blocked = compile(
        r#"{:hierarchical true :order [f s m mid n a b]
      :classes [{:id :s :level 3} {:id :m :level 2} {:id :mid :level 2} {:id :n :level 1}
                {:id :a :level 0} {:id :b :level 0} {:id :f.s :level 3}]
      :edges [{:from :s :to :a :kind :dependency} {:from :s :to :b :kind :dependency}]}"#,
    );
    assert!(unbundled(&blocked), "base routes retained");
}

#[test]
fn direction_style_and_relation_kind_stay_isolated() {
    for text in [
        "{:hierarchical true :order [a b c] :classes [{:id :a :level 2} {:id :b :level 2} {:id :c :level 3}] :edges [{:from :a :to :c :kind :dependency} {:from :c :to :b :kind :dependency}]}",
        "{:hierarchical true :order [a b c] :classes [{:id :a :level 2} {:id :b :level 2} {:id :c :level 3}] :edges [{:from :a :to :c :kind :dependency :violating true} {:from :b :to :c :kind :dependency}]}",
        "{:hierarchical true :order [a b c] :classes [{:id :a :level 2} {:id :b :level 2} {:id :c :level 3}] :edges [{:from :a :to :c :kind :dependency} {:from :b :to :c :kind :association}]}",
    ] {
        assert!(unbundled(&compile(text)), "{text}");
    }
}

#[test]
fn foreign_and_unranked_endpoints_are_not_grouped_by_level() {
    let foreign = compile(
        r#"{:hierarchical true :order [a b c ext]
      :classes [{:id :ext :foreign true} {:id :a :level 3} {:id :b :level 3} {:id :c :level 3}]
      :edges [{:from :a :to :ext :kind :dependency} {:from :b :to :ext :kind :dependency}
              {:from :c :to :ext :kind :dependency}]}"#,
    );
    assert!(
        unbundled(&foreign),
        "parallel foreign routes are not merged"
    );
    let unranked = compile(
        r#"{:packages [{:id :p :classes [{:id :app} {:id :a} {:id :b} {:id :c}]}]
      :edges [{:from :app :to :a :kind :dependency} {:from :app :to :b :kind :dependency}
              {:from :app :to :c :kind :dependency}]}"#,
    );
    assert!(unbundled(&unranked), "unranked fan-out keeps base routes");
}

#[test]
fn many_parallel_dependencies_form_real_bounded_trunks_deterministically() {
    let mut classes = String::new();
    let mut edges = String::new();
    for m in 0..40 {
        classes += &format!("{{:id :app{m} :level 1}} ");
        for t in 0..4 {
            classes += &format!("{{:id :t{m}x{t} :level 3}} ");
            edges += &format!("{{:from :app{m} :to :t{m}x{t} :kind :dependency}} ");
        }
    }
    let text = format!("{{:hierarchical true :classes [{classes}] :edges [{edges}]}}");
    let scene = compile(&text);
    let counts = member_counts(&scene);
    assert!(
        counts.len() >= 40,
        "160 parallel dependencies produce trunks, got {counts:?}"
    );
    assert!(
        counts.iter().any(|n| *n >= 3),
        "multi-member trunks: {counts:?}"
    );
    assert!(
        counts.iter().all(|n| *n <= 12),
        "members stay bounded: {counts:?}"
    );
    assert_eq!(160, seq(get(&scene, "edges")).len());
    assert_eq!(
        scene,
        compile(&text),
        "attachment, branch and member order repeat"
    );
}
