//! Every long edge of the self-hosted diagram keeps its reserved lanes:
//! no view routes one through a fallback candidate (#185).

use edn::Value;
use std::path::Path;

#[test]
fn no_view_of_the_self_hosted_diagram_drops_a_lane() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/uml-viewer.edn");
    let doc = scene::ir::read_document(&std::fs::read_to_string(path).unwrap()).unwrap();
    for focus in [
        "",
        ":companion",
        ":scene",
        ":viewer",
        ":ir",
        ":edn",
        ":main",
    ] {
        let focus: Vec<Value> = (!focus.is_empty())
            .then(|| Value::keyword(&focus[1..]))
            .into_iter()
            .collect();
        let scene = scene::document::compile_view(&doc, &[], &focus, &Value::Map(vec![]));
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
