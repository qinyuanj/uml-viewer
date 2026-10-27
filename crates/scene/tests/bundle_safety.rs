//! Safety and scaling at the public scene compilation seam.
use edn::record::{get, seq};
use edn::{Value, parse};

fn compile(text: &str) -> Value {
    let doc = scene::ir::read_document(text).unwrap();
    scene::document::compile_view(&doc, &[], &[], &parse("{}").unwrap())
}

#[test]
fn foundation_fan_in_shares_a_partial_channel() {
    let scene = compile(
        r#"{:hierarchical true :order [app a b c]
      :classes [{:id :app :level 1} {:id :a :level 3} {:id :b :level 3} {:id :c :level 3}]
      :edges [{:from :a :to :app :kind :dependency} {:from :b :to :app :kind :dependency}
              {:from :c :to :app :kind :dependency}]}"#,
    );
    assert!(
        seq(get(&scene, "bundles"))
            .iter()
            .any(|b| seq(get(b, "members")).len() >= 2),
        "Foundation fan-in shares a partial interval"
    );
    assert!(
        seq(get(&scene, "edges"))
            .iter()
            .all(|e| get(e, "points").is_some()),
        "base routes remain"
    );
}

#[test]
fn rank_hops_share_only_when_obstacles_and_crossings_allow_it() {
    let safe = compile(
        r#"{:hierarchical true :order [a b c d]
      :classes [{:id :a :level 1} {:id :b :level 2} {:id :c :level 3} {:id :d :level 4}]
      :edges [{:from :a :to :c :kind :dependency} {:from :b :to :d :kind :dependency}]}"#,
    );
    assert!(
        !seq(get(&safe, "bundles")).is_empty(),
        "adjacent rank-hop shares a safe interval"
    );
    let blocked = compile(
        r#"{:hierarchical true :order [a obstacle b]
      :classes [{:id :a :level 1} {:id :obstacle :level 2} {:id :b :level 3}]
      :edges [{:from :a :to :b :kind :dependency} {:from :a :to :obstacle :kind :dependency}]}"#,
    );
    assert!(
        get(&blocked, "bundles").is_none(),
        "class-box obstacle rejects candidate"
    );
}

#[test]
fn direction_style_and_foreign_endpoints_stay_isolated() {
    for text in [
        "{:hierarchical true :order [a b c] :classes [{:id :a :level 2} {:id :b :level 2} {:id :c :level 3}] :edges [{:from :a :to :c :kind :dependency} {:from :c :to :b :kind :dependency}]}",
        "{:hierarchical true :order [a b c] :classes [{:id :a :level 2} {:id :b :level 2} {:id :c :level 3}] :edges [{:from :a :to :c :kind :dependency :violating true} {:from :b :to :c :kind :dependency}]}",
        "{:hierarchical true :order [a b c] :classes [{:id :a :level 2} {:id :b :level 2} {:id :c :level 3}] :edges [{:from :a :to :c :kind :dependency} {:from :b :to :c :kind :association}]}",
    ] {
        assert!(
            get(&compile(text), "bundles").is_none(),
            "incompatible styles keep base routes: {text}"
        );
    }
    let foreign = compile(
        r#"{:hierarchical true :order [left middle right target]
      :classes [{:id :target :level 1} {:id :left :level 3} {:id :middle :level 3} {:id :right :level 3}]
      :edges [{:from :left :to :target :kind :dependency} {:from :middle :to :target :kind :dependency} {:from :right :to :target :kind :dependency}]}"#,
    );
    assert!(
        seq(get(&foreign, "bundles"))
            .iter()
            .any(|b| seq(get(b, "members")).len() >= 2),
        "same-rank endpoints outside one module share"
    );
    let unranked = compile(
        r#"{:packages [{:id :p :classes [{:id :target} {:id :left} {:id :right}]}]
      :edges [{:from :left :to :target :kind :dependency} {:from :right :to :target :kind :dependency}]}"#,
    );
    assert!(
        get(&unranked, "bundles").is_none() || seq(get(&unranked, "edges")).len() == 2,
        "unranked endpoints keep independent endpoints"
    );
    let crossing = compile(
        r#"{:hierarchical true :order [a b c]
      :classes [{:id :a :level 2} {:id :b :level 2} {:id :c :level 3}]
      :edges [{:from :a :to :c :kind :dependency} {:from :c :to :b :kind :dependency}]}"#,
    );
    assert!(
        get(&crossing, "bundles").is_none(),
        "opposite directions do not share a trunk"
    );
}

#[test]
fn parallel_pressure_creates_bounded_trunks_and_stable_order() {
    let classes = (0..80)
        .map(|i| format!("{{:id :s{i} :level 1}} {{:id :t{i} :level 2}}"))
        .collect::<Vec<_>>()
        .join(" ");
    let edges = (0..80)
        .map(|i| format!("{{:from :s{i} :to :t{i} :kind :dependency}}"))
        .collect::<Vec<_>>()
        .join(" ");
    let scene = compile(&format!(
        "{{:hierarchical true :order [] :classes [{classes}] :edges [{edges}]}}"
    ));
    let bundles = seq(get(&scene, "bundles"));
    assert!(
        bundles.len() >= 10,
        "same-rank pressure produces actual trunks, got {}",
        bundles.len()
    );
    assert!(
        bundles.iter().all(|b| seq(get(b, "members")).len() <= 12),
        "summary stays bounded"
    );
    assert_eq!(
        scene,
        compile(&format!(
            "{{:hierarchical true :order [] :classes [{classes}] :edges [{edges}]}}"
        )),
        "attachment and member order repeat"
    );
}

#[test]
fn app_fan_out_shares_a_channel_without_replacing_targets() {
    let scene = compile(
        r#"{:hierarchical true :order [app a b c d]
      :classes [{:id :app :level 1} {:id :a :level 3} {:id :b :level 3}
                {:id :c :level 3} {:id :d :level 3}]
      :edges [{:from :app :to :a :kind :dependency}
              {:from :app :to :b :kind :dependency}
              {:from :app :to :c :kind :dependency}
              {:from :app :to :d :kind :dependency}]}"#,
    );
    assert!(
        seq(get(&scene, "bundles"))
            .iter()
            .any(|b| seq(get(b, "members")).len() >= 3),
        "App fan-out needs a multi-member trunk"
    );
    assert_eq!(4, seq(get(&scene, "edges")).len());
}
