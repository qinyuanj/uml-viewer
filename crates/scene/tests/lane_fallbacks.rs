//! Every long edge of the self-hosted diagram keeps its reserved lanes:
//! no view routes one through a fallback candidate (#185).

mod common;

#[test]
fn no_view_of_the_self_hosted_diagram_drops_a_lane() {
    for (focus, scene) in common::self_hosted_views() {
        let edges = edn::record::seq(edn::record::get(&scene, "edges"));
        let dropped: Vec<_> = edges
            .iter()
            .filter(|e| edn::record::flag(e, "lane-fallback"))
            .map(|e| (edn::record::get(e, "from"), edn::record::get(e, "to")))
            .collect();
        assert!(!edges.is_empty(), "{focus:?} has edges");
        assert!(dropped.is_empty(), "{focus:?} dropped lanes: {dropped:?}");
    }
}
