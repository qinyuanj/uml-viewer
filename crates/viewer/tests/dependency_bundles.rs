//! Dependency bundles through the existing scene -> drawing command boundary.

use edn::Value;
use edn::record::{get, seq};
use scene::document::ViewState;
use viewer::commands::{self, Command, EdgeCommand};
use viewer::theme;

fn document() -> Value {
    scene::ir::read_document(include_str!(
        "../../../fixtures/bundle-goldens/shared-channel.ir.edn"
    ))
    .unwrap()
}

fn state() -> ViewState {
    scene::document::new_state(document(), None, None, None, &[], None)
}

fn trunk(commands: &[EdgeCommand]) -> &EdgeCommand {
    commands
        .iter()
        .find(|c| {
            matches!(
                c,
                EdgeCommand::Stroke {
                    from: None,
                    to: None,
                    ..
                }
            )
        })
        .expect("one explicit trunk stroke")
}

#[test]
fn shared_trunk_is_drawn_once_without_an_extra_head_and_chain_keeps_branches_independent() {
    let mut s = state();
    let plain = commands::edge_commands(&s.scene, None, None);
    let EdgeCommand::Stroke { points, .. } = trunk(&plain) else {
        unreachable!()
    };
    assert_eq!(
        vec![[288.0, 188.66666666666666], [288.0, 235.33333333333331]],
        *points
    );
    assert_eq!(
        1,
        plain
            .iter()
            .filter(|c| matches!(c, EdgeCommand::Stroke { from: None, .. }))
            .count()
    );
    assert_eq!(
        2,
        plain
            .iter()
            .filter(|c| matches!(c, EdgeCommand::Head { .. }))
            .count()
    );
    assert_eq!(
        4,
        plain
            .iter()
            .filter(|c| matches!(c, EdgeCommand::Stroke { from: Some(_), .. }))
            .count()
    );

    s.selected = Some(edn::parse("{:kind :class :id :d}").unwrap());
    let selected = commands::edge_commands(&s.scene, s.selected.as_ref(), None);
    assert!(
        matches!(trunk(&selected), EdgeCommand::Stroke { color, dimmed: false, .. } if *color == theme::RELATED)
    );
    for command in &selected {
        if let EdgeCommand::Stroke {
            from: Some(from),
            color,
            dimmed,
            ..
        } = command
        {
            if *from == Value::keyword("d") {
                assert_eq!(theme::RELATED, *color);
                assert!(!dimmed);
            } else {
                assert_eq!(theme::EDGE, *color);
                assert!(dimmed, "sharing a trunk does not enter the chain");
            }
        }
    }
}

fn popup(state: &ViewState, height: f64) -> Option<(f64, Vec<commands::PopupRow>)> {
    commands::chrome(state, 800.0, height)
        .into_iter()
        .find_map(|c| match c {
            Command::DependencyPopup { h, rows, .. } => Some((h, rows)),
            _ => None,
        })
}

#[test]
fn trunk_hover_lists_visible_pairs_and_directly_hovers_members_then_leaving_restores_selection() {
    let mut s = state();
    s.selected = Some(edn::parse("{:kind :class :id :d}").unwrap());
    s.hover = scene::hit::at(&s.scene, 288.0, 210.0);
    s.pointer = Some((300.0, 240.0));
    let (_, rows) = popup(&s, 600.0).expect("trunk member summary");
    assert_eq!(
        vec!["c -> a", "d -> e"],
        rows.iter().map(|r| r.text.as_str()).collect::<Vec<_>>()
    );
    let hovered = commands::edge_commands(&s.scene, s.selected.as_ref(), s.hover.as_ref());
    assert!(
        hovered.iter().all(|c| match c {
            EdgeCommand::Stroke {
                color, hot, dimmed, ..
            }
            | EdgeCommand::Head {
                color, hot, dimmed, ..
            } => *color == theme::WHITE && *hot && !dimmed,
            _ => false,
        }),
        "trunk hover uses direct-edge attention on every member"
    );

    s.hover = scene::hit::at(&s.scene, 448.0, 160.0);
    let (_, rows) = popup(&s, 600.0).expect("branch leaf details");
    assert_eq!(
        vec!["d -> e"],
        rows.iter().map(|r| r.text.as_str()).collect::<Vec<_>>()
    );

    s = scene::events::on_leave(&s);
    assert!(s.hover.is_none() && s.pointer.is_none());
    assert!(popup(&s, 600.0).is_none());
    let restored = commands::edge_commands(&s.scene, s.selected.as_ref(), None);
    assert!(
        matches!(trunk(&restored), EdgeCommand::Stroke { color, hot: false, .. } if *color == theme::RELATED)
    );
}

#[test]
fn trunk_summary_is_bounded_by_viewport_and_reports_the_remaining_visible_pairs() {
    let mut s = state();
    let members = (0..40)
        .map(|i| {
            edn::record::record(vec![
                ("from", Value::keyword(&format!("source{i:02}"))),
                ("to", Value::keyword("target")),
                ("violating", Value::Bool(i == 39)),
            ])
        })
        .collect();
    s.hover = Some(edn::record::record(vec![
        ("kind", Value::keyword("bundle")),
        ("members", Value::Vector(members)),
    ]));
    s.pointer = Some((200.0, 40.0));
    let (h, rows) = popup(&s, 160.0).unwrap();
    assert!(h <= 144.0, "summary remains within viewport: {h}");
    assert_eq!(
        "source39 -> target", rows[0].text,
        "violations remain visible first"
    );
    assert_eq!("另有 35 对可见依赖", rows.last().unwrap().text);
    assert_eq!(6, rows.len(), "five members and one remainder row");
}

#[test]
fn violating_bundle_keeps_red_attention_and_remove_arrows_keeps_original_dependency_triangles() {
    let mut doc = document();
    let mut edges = seq(get(&doc, "edges")).to_vec();
    for e in &mut edges {
        edn::record::put(e, "violating", Value::Bool(true));
    }
    edn::record::put(&mut doc, "edges", Value::Vector(edges));
    let mut s = scene::document::new_state(doc, None, None, None, &[], None);
    let plain = commands::edge_commands(&s.scene, None, None);
    assert!(
        matches!(trunk(&plain), EdgeCommand::Stroke { color, .. } if *color == theme::EDGE_VIOLATING)
    );
    s.hover = scene::hit::at(&s.scene, 288.0, 210.0);
    let hovered = commands::edge_commands(&s.scene, None, s.hover.as_ref());
    assert!(hovered.iter().all(|c| match c {
        EdgeCommand::Stroke { color, hot, .. } | EdgeCommand::Head { color, hot, .. } =>
            *color == theme::EDGE_VIOLATING_HOT && *hot,
        _ => false,
    }));
    let original_deps: usize = seq(get(&s.scene, "edges"))
        .iter()
        .map(|e| seq(get(e, "deps")).len())
        .sum();
    s = scene::events::cycle_declutter(&s);
    s = scene::events::cycle_declutter(&s);
    let hidden = commands::edge_commands(&s.scene, None, None);
    assert_eq!(4, hidden.len());
    assert!(hidden.iter().all(|c| matches!(
        c,
        EdgeCommand::Indicator {
            violating: true,
            ..
        }
    )));
    let indicator_deps: usize = seq(get(&s.scene, "dep-indicators"))
        .iter()
        .map(|i| seq(get(i, "deps")).len())
        .sum();
    assert_eq!(
        original_deps * 2,
        indicator_deps,
        "each original leaf appears at both endpoints"
    );
    assert_ne!(
        Some(&Value::keyword("bundle")),
        scene::hit::at(&s.scene, 288.0, 210.0)
            .as_ref()
            .and_then(|h| get(h, "kind")),
        "Remove arrows does not hit hidden trunks"
    );
}
