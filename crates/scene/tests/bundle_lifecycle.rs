//! 当前视图重编译后的依赖线束生命周期。
use edn::Value;
use edn::record::{get, seq};
use scene::{document, events};

const FIXTURE: &str = include_str!("../../../fixtures/bundle_lifecycle.ir.edn");
/// The fixture pins bundles planned on the base layout, like the historical
/// goldens: the span order (#183) uncrowds exactly these fan-outs.
fn state() -> document::ViewState {
    document::with_rank_order(
        document::new_state(
            scene::ir::read_document(FIXTURE).unwrap(),
            None,
            None,
            None,
            &[],
            None,
        ),
        scene::RankOrder::Base,
    )
}

#[test]
fn declutter_discards_previous_bundle_hover_when_rebuilding_current_geometry() {
    let mut s = state();
    assert!(!seq(get(&s.scene, "bundles")).is_empty(), "真实非空主干");
    let bundle = &seq(get(&s.scene, "bundles"))[0];
    let strokes = seq(get(bundle, "strokes"));
    let points = seq(Some(&strokes[0]));
    let p = seq(Some(&points[0]));
    let q = seq(Some(&points[1]));
    let x = (edn::record::num(&p[0]).unwrap() + edn::record::num(&q[0]).unwrap()) / 2.0;
    let y = (edn::record::num(&p[1]).unwrap() + edn::record::num(&q[1]).unwrap()) / 2.0;
    s.hover = scene::hit::at(&s.scene, x, y);
    s.pointer = Some((x, y));
    assert_eq!(
        Some(&Value::keyword("bundle")),
        s.hover.as_ref().and_then(|h| get(h, "kind"))
    );
    let arrows = events::cycle_declutter(&s);
    assert!(arrows.hover.is_none(), "重编译不能保留上一场景的主干成员");
    assert!(arrows.pointer.is_none());
}

fn kw(s: &str) -> Value {
    Value::keyword(s)
}

fn assert_current(s: &document::ViewState) {
    let compiled = document::compile_view_with(
        s.document.as_ref().unwrap(),
        &[],
        &s.focus,
        &s.extra,
        s.rank_order,
    );
    assert_eq!(
        compiled, s.scene,
        "当前视图独立编译，包含端点、分支、头部与成员顺序"
    );
    for bundle in seq(get(&s.scene, "bundles")) {
        for member in seq(get(bundle, "members")) {
            let index = edn::record::num(member).unwrap() as usize;
            let edge = &seq(get(&s.scene, "edges"))[index];
            for key in ["from", "to"] {
                assert!(
                    seq(get(&s.scene, "classes"))
                        .iter()
                        .any(|c| get(c, "id") == get(edge, key))
                );
            }
            assert!(!seq(get(edge, "deps")).is_empty());
            assert!(get(edge, "tip").is_some());
        }
    }
}

#[test]
fn source_and_proposal_navigation_rebuild_nonempty_bundles_and_restore_root() {
    let root = state();
    assert_current(&root);
    let child = events::drill(&root, &kw("f"));
    assert_current(&child);
    assert!(
        !seq(get(&child.scene, "bundles")).is_empty(),
        "子层必须有真实线束"
    );
    assert_ne!(get(&root.scene, "edges"), get(&child.scene, "edges"));
    assert_eq!(root.scene, events::back(&child).scene);
    let proposal = events::show_proposal(&root, Some(&kw("plan")));
    assert_current(&proposal);
    let opened = events::drill(&proposal, &kw("proposal.shell"));
    assert_current(&opened);
    assert!(
        !seq(get(&opened.scene, "bundles")).is_empty(),
        "提案下一层必须有真实线束"
    );
    assert_eq!(proposal.scene, events::back(&opened).scene);
    assert_eq!(root.scene, events::show_proposal(&proposal, None).scene);
}

#[test]
fn reload_recompile_and_camera_preserve_world_geometry_and_member_order() {
    let root = state();
    for s in [root.clone(), events::drill(&root, &kw("f"))] {
        let (reloaded, changed, error) = document::reload(&s, FIXTURE, Some(kw("new")), &[], None);
        assert!(changed && error.is_none());
        assert_eq!(s.scene, reloaded.scene);
        let zoomed = events::zoom_at(&s, 1.1, 320.0, 240.0);
        let moved = events::on_wheel(&zoomed, 0.0, -2.0, &events::Dims::default());
        let resized = events::fit(
            &moved,
            &events::Dims {
                window_w: 900.0,
                window_h: 700.0,
                ..Default::default()
            },
        );
        assert_ne!(s.zoom, zoomed.zoom);
        assert_eq!(s.scene, resized.scene);
        for _ in 0..4 {
            assert_current(&s);
        }
    }
}

fn leaves(s: &document::ViewState) -> Vec<String> {
    let mut deps: Vec<_> = seq(get(&s.scene, "edges"))
        .iter()
        .flat_map(|e| seq(get(e, "deps")))
        .map(|dep| edn::encode(dep).unwrap())
        .collect();
    deps.sort();
    deps
}

#[test]
fn every_declutter_preserves_leaf_dependencies_and_remove_arrows_triangle_details() {
    let root = state();
    for start in [root.clone(), events::drill(&root, &kw("f"))] {
        let expected = leaves(&start);
        let mut current = start.clone();
        for mode in ["arrows", "triangles", "elements", "classes", "full"] {
            current = events::cycle_declutter(&current);
            assert_eq!(Some(&kw(mode)), get(&current.extra, "declutter"));
            assert_eq!(expected, leaves(&current), "{mode} 的 leaf 明细");
            assert_current(&current);
            if mode == "triangles" {
                let commands = viewer_free_indicators(&current);
                assert_eq!(expected.len() * 2, commands);
                assert_ne!(
                    Some(&kw("bundle")),
                    scene::hit::at(&current.scene, 288.0, 210.0)
                        .as_ref()
                        .and_then(|h| get(h, "kind"))
                );
            } else {
                assert!(
                    !seq(get(&current.scene, "bundles")).is_empty(),
                    "{mode} 保留线束"
                );
            }
        }
        assert_eq!(start.scene, current.scene);
    }
}

fn viewer_free_indicators(s: &document::ViewState) -> usize {
    seq(get(&s.scene, "dep-indicators"))
        .iter()
        .map(|i| seq(get(i, "deps")).len())
        .sum()
}
