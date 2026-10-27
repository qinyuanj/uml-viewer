//! #181 组合验收：多成员、跨 rank、障碍回退、违规、源码图/提案与视图生命周期
//! 组合在同一份 hierarchical IR 上时，每个当前视图的逻辑边、端点、头部、
//! 命中、悬停、依赖链和违规样式仍彼此一致。断言只读公共场景、命中与绘制命令。

use edn::Value;
use edn::record::{flag, get, local_name, num, seq};
use scene::document::{self, ViewState};
use scene::events;
use std::collections::{BTreeMap, BTreeSet};
use viewer::commands::{self, Command, EdgeCommand};
use viewer::theme;

const FIXTURE: &str = include_str!("../../../fixtures/bundle_acceptance.ir.edn");
const EPS: f64 = 1e-6;

fn kw(s: &str) -> Value {
    Value::keyword(s)
}

fn state() -> ViewState {
    document::new_state(
        scene::ir::read_document(FIXTURE).unwrap(),
        None,
        None,
        None,
        &[],
        None,
    )
}

fn xy(v: &Value) -> Option<[f64; 2]> {
    let p = seq(Some(v));
    Some([num(p.first()?)?, num(p.get(1)?)?])
}

fn paths(v: Option<&Value>) -> Vec<Vec<[f64; 2]>> {
    seq(v)
        .iter()
        .map(|path| seq(Some(path)).iter().filter_map(xy).collect())
        .collect()
}

fn same_point(a: [f64; 2], b: [f64; 2]) -> bool {
    (a[0] - b[0]).abs() <= EPS && (a[1] - b[1]).abs() <= EPS
}

fn name(v: Option<&Value>) -> String {
    v.and_then(local_name).unwrap_or_default().to_owned()
}

fn pair(e: &Value) -> (String, String) {
    (name(get(e, "from")), name(get(e, "to")))
}

fn violating(e: &Value) -> bool {
    flag(e, "violating") || seq(get(e, "deps")).iter().any(|d| flag(d, "violating"))
}

fn is_dependency(v: &Value) -> bool {
    get(v, "kind").and_then(local_name) == Some("dependency")
}

fn rank(scene: &Value, id: Option<&Value>) -> Option<f64> {
    seq(get(scene, "classes"))
        .iter()
        .find(|c| get(c, "id") == id)
        .and_then(|c| get(c, "rank").and_then(num))
}

fn members(scene: &Value, bundle: &Value) -> Vec<usize> {
    let _ = scene;
    seq(get(bundle, "members"))
        .iter()
        .map(|m| num(m).unwrap() as usize)
        .collect()
}

/// What a view actually combines, so a scenario cannot pass on empty trunks.
#[derive(Debug, Default)]
struct Combination {
    trunks: usize,
    widest: usize,
    violating_trunk: bool,
    plain_trunk: bool,
    rank_hop_member: bool,
    fallback_dependency: bool,
}

/// The classes the selection depends on, directly or transitively, along leaf
/// dependencies (GLOSSARY 依赖链), and whether an edge carries one of them.
fn chain(scene: &Value, selected: &Value) -> BTreeSet<String> {
    let mut reach = BTreeSet::from([name(Some(selected))]);
    loop {
        let before = reach.len();
        for e in seq(get(scene, "edges")) {
            for dep in seq(get(e, "deps")) {
                if is_dependency(dep) && reach.contains(&name(get(dep, "from"))) {
                    reach.insert(name(get(dep, "to")));
                }
            }
        }
        if reach.len() == before {
            return reach;
        }
    }
}

fn on_chain(e: &Value, reach: &BTreeSet<String>) -> bool {
    seq(get(e, "deps"))
        .iter()
        .any(|d| is_dependency(d) && reach.contains(&name(get(d, "from"))))
}

fn violating_color(c: theme::Rgb) -> bool {
    c == theme::EDGE_VIOLATING || c == theme::EDGE_VIOLATING_HOT
}

fn popup_rows(s: &ViewState) -> Option<Vec<String>> {
    commands::chrome(s, 1200.0, 900.0)
        .into_iter()
        .find_map(|c| match c {
            Command::DependencyPopup { rows, .. } => {
                Some(rows.into_iter().map(|r| r.text).collect())
            }
            _ => None,
        })
}

/// Every invariant of #172 that must hold in whatever view is current.
fn assert_consistent(s: &ViewState, label: &str) -> Combination {
    let scene = &s.scene;
    let edges = seq(get(scene, "edges"));
    let bundles = seq(get(scene, "bundles"));
    let mut found = Combination {
        trunks: bundles.len(),
        ..Default::default()
    };

    // 同一对可见模块、同一方向只有一根逻辑边；每条边保留 leaf 明细、头部与端点。
    let pairs: BTreeSet<_> = edges.iter().map(pair).collect();
    assert_eq!(
        edges.len(),
        pairs.len(),
        "{label}: one logical edge per pair"
    );
    for e in edges {
        assert!(
            !seq(get(e, "deps")).is_empty(),
            "{label}: {:?} leaves",
            pair(e)
        );
        assert!(get(e, "head").is_some() && get(e, "tip").is_some());
        for end in ["from", "to"] {
            assert!(
                rank(scene, get(e, end)).is_some()
                    || seq(get(scene, "classes"))
                        .iter()
                        .any(|c| get(c, "id") == get(e, end)),
                "{label}: {:?} keeps a visible {end}",
                pair(e)
            );
        }
    }

    // 主干：至少两名成员，全为同违规样式的普通依赖，无反向成员，每名成员有分支。
    let mut carried = BTreeSet::new();
    for (b, bundle) in bundles.iter().enumerate() {
        let ms = members(scene, bundle);
        assert!(ms.len() >= 2, "{label}: trunk {b} has {} members", ms.len());
        let set: BTreeSet<_> = ms.iter().copied().collect();
        assert_eq!(ms.len(), set.len(), "{label}: trunk {b} members distinct");
        let style = violating(&edges[ms[0]]);
        for &m in &ms {
            let e = &edges[m];
            assert!(
                is_dependency(e),
                "{label}: trunk {b} carries only dependencies"
            );
            assert_eq!(style, violating(e), "{label}: trunk {b} keeps one style");
            assert!(get(e, "branches").is_some(), "{label}: member {m} branches");
            let (from, to) = pair(e);
            assert!(
                !ms.iter()
                    .any(|&o| pair(&edges[o]) == (to.clone(), from.clone())),
                "{label}: trunk {b} mixes {from}->{to} with its reverse"
            );
            let hop = rank(scene, get(e, "from"))
                .zip(rank(scene, get(e, "to")))
                .is_some_and(|(f, t)| (f - t).abs() > 1.0);
            found.rank_hop_member |= hop;
            carried.insert(m);
        }
        found.widest = found.widest.max(ms.len());
        found.violating_trunk |= style;
        found.plain_trunk |= !style;
    }
    for (i, e) in edges.iter().enumerate() {
        assert_eq!(
            carried.contains(&i),
            get(e, "branches").is_some(),
            "{label}: {:?} branches iff a trunk carries it",
            pair(e)
        );
        found.fallback_dependency |= is_dependency(e) && !carried.contains(&i);
        // 分支仍从原起点出发、到原终点结束：没有改接到代表模块。
        if get(e, "branches").is_some() {
            let base = paths(get(e, "strokes"));
            let branch = paths(get(e, "branches"));
            assert!(
                same_point(base[0][0], branch[0][0]),
                "{label}: {:?} start",
                pair(e)
            );
            assert!(
                same_point(
                    *base.last().unwrap().last().unwrap(),
                    *branch.last().unwrap().last().unwrap()
                ),
                "{label}: {:?} end",
                pair(e)
            );
        }
    }

    let hidden = get(scene, "diagram").is_some_and(|d| flag(d, "hide-edges"));
    let plain = commands::edge_commands(scene, None, None);
    if hidden {
        // Remove arrows：没有主干、分支或头部，三角仍由原逻辑边的 leaf 生成。
        assert!(
            plain
                .iter()
                .all(|c| matches!(c, EdgeCommand::Indicator { .. }))
        );
        let leaves: usize = edges.iter().map(|e| seq(get(e, "deps")).len()).sum();
        let shown: usize = seq(get(scene, "dep-indicators"))
            .iter()
            .map(|i| seq(get(i, "deps")).len())
            .sum();
        assert_eq!(leaves * 2, shown, "{label}: every leaf at both ends");
        return found;
    }

    // 绘制：每段主干只画一次，没有自己的头部；每条逻辑边各有一个头部。
    let trunk_strokes: usize = bundles.iter().map(|b| seq(get(b, "strokes")).len()).sum();
    let drawn_trunks = plain
        .iter()
        .filter(|c| matches!(c, EdgeCommand::Trunk { .. }))
        .count();
    assert_eq!(trunk_strokes, drawn_trunks, "{label}: trunks drawn once");
    let mut heads: BTreeMap<(String, String), usize> = BTreeMap::new();
    for c in &plain {
        if let EdgeCommand::Head { from, to, .. } = c {
            assert!(
                from.is_some() && to.is_some(),
                "{label}: trunk without head"
            );
            *heads
                .entry((name(from.as_ref()), name(to.as_ref())))
                .or_default() += 1;
        }
    }
    assert_eq!(
        pairs
            .iter()
            .map(|p| (p.clone(), 1))
            .collect::<BTreeMap<_, _>>(),
        heads,
        "{label}: one head per logical edge"
    );

    // 命中：主干返回全部成员，分支返回自身逻辑边及 leaf 明细。
    for (b, bundle) in bundles.iter().enumerate() {
        let expected: BTreeSet<_> = members(scene, bundle)
            .iter()
            .map(|&m| pair(&edges[m]))
            .collect();
        for stroke in paths(get(bundle, "strokes")) {
            let mid = [
                (stroke[0][0] + stroke[1][0]) / 2.0,
                (stroke[0][1] + stroke[1][1]) / 2.0,
            ];
            let hit = scene::hit::at(scene, mid[0], mid[1]).expect("trunk hit");
            assert_eq!(Some("bundle"), get(&hit, "kind").and_then(local_name));
            let got: BTreeSet<_> = seq(get(&hit, "members")).iter().map(pair).collect();
            assert_eq!(expected, got, "{label}: trunk {b} hit lists its members");

            // 悬停主干：弹窗按可见模块对摘要，成员按直接悬停呈现，违规保持红色。
            let mut hovered = s.clone();
            hovered.hover = Some(hit.clone());
            hovered.pointer = Some((mid[0], mid[1]));
            let rows: BTreeSet<_> = popup_rows(&hovered)
                .expect("trunk popup")
                .into_iter()
                .collect();
            let wanted: BTreeSet<_> = expected
                .iter()
                .map(|(f, t)| format!("{f} -> {t}"))
                .collect();
            assert_eq!(wanted, rows, "{label}: trunk {b} popup");
            for c in commands::edge_commands(scene, None, hovered.hover.as_ref()) {
                if let EdgeCommand::Stroke {
                    from: Some(f),
                    to: Some(t),
                    hot,
                    color,
                    ..
                }
                | EdgeCommand::Head {
                    from: Some(f),
                    to: Some(t),
                    hot,
                    color,
                    ..
                } = c
                {
                    let p = (name(Some(&f)), name(Some(&t)));
                    assert_eq!(expected.contains(&p), hot, "{label}: {p:?} hot");
                    let e = edges.iter().find(|e| pair(e) == p).unwrap();
                    assert_eq!(violating(e), violating_color(color), "{label}: {p:?} red");
                }
            }
            let left = events::on_leave(&hovered);
            assert!(popup_rows(&left).is_none(), "{label}: leaving clears popup");
        }
    }
    for e in edges.iter().filter(|e| get(e, "branches").is_some()) {
        let hit = paths(get(e, "branches"))
            .iter()
            .flat_map(|p| {
                p.windows(2)
                    .map(|w| [(w[0][0] + w[1][0]) / 2.0, (w[0][1] + w[1][1]) / 2.0])
                    .collect::<Vec<_>>()
            })
            .filter_map(|m| scene::hit::at(scene, m[0], m[1]))
            .find(|h| get(h, "kind").and_then(local_name) == Some("edge") && pair(h) == pair(e));
        let hit = hit.unwrap_or_else(|| panic!("{label}: {:?} branch hittable", pair(e)));
        assert_eq!(
            get(e, "deps"),
            get(&hit, "deps"),
            "{label}: branch leaf detail"
        );
    }

    // 依赖链：主干只在有成员在链上时进入链，分支按各自成员关系，违规仍为红。
    let mut sources: Vec<Value> = Vec::new();
    for m in bundles.iter().flat_map(|b| members(scene, b)) {
        let from = get(&edges[m], "from").cloned().unwrap();
        if !sources.contains(&from) {
            sources.push(from);
        }
    }
    for source in sources {
        let selected = edn::record::record(vec![("kind", kw("class")), ("id", source.clone())]);
        let reach = chain(scene, &source);
        let commands = commands::edge_commands(scene, Some(&selected), None);
        let mut trunk_index = 0;
        for bundle in bundles {
            let ms = members(scene, bundle);
            let any = ms.iter().any(|&m| on_chain(&edges[m], &reach));
            let red = violating(&edges[ms[0]]);
            for _ in seq(get(bundle, "strokes")) {
                let trunk = commands
                    .iter()
                    .filter(|c| matches!(c, EdgeCommand::Trunk { .. }))
                    .nth(trunk_index)
                    .unwrap();
                trunk_index += 1;
                let EdgeCommand::Trunk { dimmed, color, .. } = trunk else {
                    unreachable!()
                };
                assert_eq!(!any, *dimmed, "{label}: trunk chain from {source:?}");
                assert_eq!(
                    red,
                    violating_color(*color),
                    "{label}: trunk red from {source:?}"
                );
            }
        }
        for c in &commands {
            if let EdgeCommand::Stroke {
                from: Some(f),
                to: Some(t),
                dimmed,
                color,
                ..
            } = c
            {
                let p = (name(Some(f)), name(Some(t)));
                let e = edges.iter().find(|e| pair(e) == p).unwrap();
                assert_eq!(
                    !on_chain(e, &reach),
                    *dimmed,
                    "{label}: {p:?} chain from {source:?}"
                );
                assert_eq!(violating(e), violating_color(*color), "{label}: {p:?} red");
            }
        }
    }
    found
}

#[test]
fn the_root_combines_rank_hop_violating_and_fallback_routes_consistently() {
    let root = state();
    let found = assert_consistent(&root, "root");
    assert!(found.plain_trunk && found.violating_trunk, "{found:?}");
    assert!(found.rank_hop_member, "a trunk crosses ranks: {found:?}");
    assert!(
        found.fallback_dependency,
        "an unsafe candidate keeps its base route: {found:?}"
    );
}

#[test]
fn drilling_proposals_declutter_and_reload_rebuild_consistent_bundles_for_each_view() {
    let root = state();
    assert_consistent(&root, "root");

    let child = events::drill(&root, &kw("ui"));
    let found = assert_consistent(&child, "ui");
    assert!(
        found.widest >= 2,
        "child fan-out shares one jog-free trunk: {found:?}"
    );
    assert_ne!(get(&root.scene, "bundles"), get(&child.scene, "bundles"));
    assert_eq!(
        root.scene,
        events::back(&child).scene,
        "back restores root trunks"
    );

    let proposal = events::show_proposal(&root, Some(&kw("plan")));
    let found = assert_consistent(&proposal, "proposal");
    assert!(found.trunks > 0, "proposal bundles: {found:?}");
    let opened = events::drill(&proposal, &kw("proposal.shell"));
    let found = assert_consistent(&opened, "proposal layer");
    assert!(found.widest >= 2, "proposal layer bundles: {found:?}");
    assert_ne!(
        get(&proposal.scene, "bundles"),
        get(&opened.scene, "bundles")
    );
    assert_eq!(proposal.scene, events::back(&opened).scene);
    let source = events::show_proposal(&proposal, None);
    assert_eq!(root.scene, source.scene, "source diagram restored");

    for start in [
        root.clone(),
        child.clone(),
        proposal.clone(),
        opened.clone(),
    ] {
        let mut current = start.clone();
        for mode in ["arrows", "triangles", "elements", "classes", "full"] {
            current = events::cycle_declutter(&current);
            assert_consistent(&current, mode);
        }
        assert_eq!(start.scene, current.scene, "declutter cycle returns");
        let (reloaded, changed, error) =
            document::reload(&start, FIXTURE, Some(kw("again")), &[], None);
        assert!(changed && error.is_none());
        assert_eq!(
            start.scene, reloaded.scene,
            "same IR, same trunks and order"
        );
        let moved = events::zoom_at(&start, 1.3, 200.0, 150.0);
        assert_eq!(
            start.scene, moved.scene,
            "camera never moves world geometry"
        );
    }
}
