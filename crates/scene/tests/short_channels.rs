//! Shared runs of 35–47 px (the original #178–#181 bundle inputs) leave no
//! room for a trunk between two bends: every view keeps its base routes.

mod common;

use edn::Value;
use edn::record::{get, seq};
use scene::{document, events};

fn state(text: &str) -> document::ViewState {
    document::new_state(
        scene::ir::read_document(text).unwrap(),
        None,
        None,
        None,
        &[],
        None,
    )
}

fn assert_base_routes(scene: &Value, label: &str) {
    assert!(get(scene, "bundles").is_none(), "{label}: no bundles");
    let edges = seq(get(scene, "edges"));
    assert!(!edges.is_empty(), "{label}: edges kept");
    for e in edges {
        assert!(get(e, "branches").is_none(), "{label}: no branches");
        assert!(
            !seq(get(e, "strokes")).is_empty(),
            "{label}: base strokes drawn"
        );
    }
}

fn read(dir: Option<&str>, name: &str) -> String {
    let path = match dir {
        Some(dir) => common::fixture(dir, name),
        None => common::repo().join("fixtures").join(name),
    };
    std::fs::read_to_string(path).unwrap()
}

#[test]
fn a_short_shared_channel_keeps_its_base_routes() {
    let s = state(&read(Some("bundle-goldens"), "short-channel.ir.edn"));
    assert_base_routes(&s.scene, "short channel");
}

#[test]
fn short_channels_keep_base_routes_in_every_lifecycle_view() {
    let root = state(&read(None, "bundle_lifecycle_short.ir.edn"));
    assert_base_routes(&root.scene, "root");
    assert_base_routes(&events::drill(&root, &Value::keyword("f")).scene, "child");
    let proposal = events::show_proposal(&root, Some(&Value::keyword("plan")));
    assert_base_routes(&proposal.scene, "proposal");
    assert_base_routes(
        &events::drill(&proposal, &Value::keyword("proposal.unassigned")).scene,
        "proposal layer",
    );
}

#[test]
fn a_one_rank_fan_hop_without_a_long_channel_keeps_its_base_routes() {
    let s = state(
        r#"{:hierarchical true :order [app a b c]
      :classes [{:id :app :level 1} {:id :a :level 4} {:id :b :level 4} {:id :c :level 4}]
      :edges [{:from :app :to :a :kind :dependency} {:from :app :to :b :kind :dependency}
              {:from :app :to :c :kind :dependency}]}"#,
    );
    assert_base_routes(&s.scene, "rank hop");
}
