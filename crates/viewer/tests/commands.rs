//! Class-box and edge command parity with the LOVE adapters
//! (`love/spec/class_boxes_spec.lua`, `love/spec/edges_spec.lua`): the
//! pure scene -> command layer, independent of any renderer.

use edn::Value;
use scene::document::compile_view;
use viewer::commands::{self, Command, EdgeCommand};

fn scene(classes: &str) -> Value {
    let text = format!("{{:packages [{{:label \"P\" :classes {classes}}}]}}");
    let doc = scene::ir::read_document(&text).unwrap();
    compile_view(&doc, &[], &[], &Value::Map(vec![]))
}

/// The root view of two modules `a` and `b` joined by `edges`, compiled
/// with `extra`; a hierarchical view keeps the authored ids.
fn two_modules(edges: &str, extra: &str) -> Value {
    let text = format!(
        "{{:hierarchical true :prefix \"x\" :order [a b] \
          :classes [{{:id :a :name \"A\" :ns \"x.a\"}} {{:id :b :name \"B\" :ns \"x.b\"}}] \
          :edges {edges}}}"
    );
    let doc = scene::ir::read_document(&text).unwrap();
    compile_view(&doc, &[], &[], &edn::parse(extra).unwrap())
}

fn commands_of(s: &Value) -> Vec<Command> {
    commands::commands(s, None, None)
}

fn of_kind<'a>(commands: &'a [Command], kind: &str) -> Vec<&'a Command> {
    commands.iter().filter(|c| c.kind() == kind).collect()
}

#[test]
fn separate_c_and_m_dots_keep_either_absent_metric_red() {
    let s = scene(
        "[{:id :a :crap {:mu 2} :killed 9 :survived 1} {:id :b :killed 1 :survived 0} {:id :c :crap {:mu 2}} {:id :d}]",
    );
    let commands = commands_of(&s);
    let mut statuses = Vec::new();
    for command in of_kind(&commands, "quality") {
        let Command::Quality {
            owner,
            rect,
            text,
            band,
            radius,
            x,
            y,
            ..
        } = command
        else {
            unreachable!()
        };
        if *owner != "class" {
            continue;
        }
        statuses.push((*text, *band));
        assert!(x - radius >= rect.x && y - radius >= rect.y);
        assert!(x + radius <= rect.right() && y + radius <= rect.bottom());
    }
    assert_eq!(
        vec![
            ("C", "green"),
            ("M", "yellow"),
            ("C", "red"),
            ("M", "green"),
            ("C", "green"),
            ("M", "red"),
            ("C", "red"),
            ("M", "red"),
        ],
        statuses
    );
}

#[test]
fn white_markers_and_level_zero_stay_inside_their_bounds() {
    let s = scene("[{:id :a :stereotype :abstract :level 0} {:id :b :stereotype :interface}]");
    let commands = commands_of(&s);
    let markers = of_kind(&commands, "marker");
    assert_eq!(2, markers.len());
    for (marker, text) in markers.iter().zip(["α", "I"]) {
        let Command::Marker {
            rect,
            text: t,
            x,
            y,
            color,
            ..
        } = marker
        else {
            unreachable!()
        };
        assert_eq!(text, t);
        assert_eq!(egui::Color32::WHITE, *color);
        assert!(*x >= rect.x && x + 8.0 < rect.right() && *y >= rect.y);
    }
    let levels = of_kind(&commands, "level");
    let Command::Level { rect, text, x, .. } = levels[0] else {
        unreachable!()
    };
    assert_eq!("0", text);
    assert_eq!(rect.x + 4.0, *x);
}

#[test]
fn italic_distinguishes_classifier_and_grouping_names() {
    // drill?/contents reach the scene through hierarchy views, not the flat
    // IR normalizer; like the busted spec, put them on the compiled classes.
    let mut s = scene(
        "[{:id :a :stereotype :interface} {:id :b :stereotype :abstract} {:id :c} {:id :d} {:id :e :stereotype :class :fields [\"field\"]}]",
    );
    if let Some(Value::Vector(classes)) = s.get(&Value::keyword("classes")).cloned() {
        let mut classes = classes;
        classes[2].assoc(Value::keyword("drill?"), Value::Bool(true));
        classes[3].assoc(
            Value::keyword("contents"),
            edn::parse("[{:id :kid :name \"Kid\"}]").unwrap(),
        );
        s.assoc(Value::keyword("classes"), Value::Vector(classes));
    }
    let commands = commands_of(&s);
    let mut styles = Vec::new();
    for command in of_kind(&commands, "text") {
        let Command::Text {
            role,
            italic,
            bold,
            text,
            size,
            ..
        } = command
        else {
            unreachable!()
        };
        if role == "name" {
            styles.push(*italic);
            assert_eq!(18.0, *size);
            assert!(bold);
        } else if text == "field" {
            assert!(!italic && !bold);
            assert_eq!(18.0, *size);
        }
    }
    assert_eq!(vec![true, true, true, true, false], styles);
}

#[test]
fn pooled_package_metrics_paint_separate_dots() {
    let s = scene(
        "[{:id :a :crap {:mu 2 :sigma 0 :n 3} :killed 9 :survived 1 :ops [\"a\" \"b\" \"c\"]} {:id :b :crap {:mu 20 :sigma 0 :n 1} :killed 1 :survived 1}]",
    );
    let commands = commands_of(&s);
    let packages = of_kind(&commands, "package");
    let Command::Package { band, text, .. } = packages[0] else {
        unreachable!()
    };
    assert_eq!("red", *band);
    assert_eq!("P", text);
    let statuses: Vec<(&str, &str)> = of_kind(&commands, "quality")
        .into_iter()
        .filter_map(|c| match c {
            Command::Quality {
                owner: "package",
                text,
                band,
                ..
            } => Some((*text, *band)),
            _ => None,
        })
        .collect();
    assert_eq!(vec![("C", "red"), ("M", "yellow")], statuses);

    let absent = scene("[{:id :a :crap 2 :killed 1 :survived 0} {:id :b :ops [\"x\" \"y\"]}]");
    let commands = commands_of(&absent);
    let Command::Package { band, .. } = of_kind(&commands, "package")[0] else {
        unreachable!()
    };
    assert_eq!("red", *band);
}

#[test]
fn unmeasured_classes_get_the_worst_grade_and_a_no_data_marker() {
    let s = scene("[{:id :a}]");
    let commands = commands_of(&s);
    let Command::Class { missing, grade, .. } = of_kind(&commands, "class")[0] else {
        unreachable!()
    };
    assert!(missing);
    assert_eq!(Some(scene::GRADE_WORST), *grade);
    let no_data = of_kind(&commands, "no-data");
    assert_eq!(1, no_data.len());
    let Command::NoData {
        text, right, rect, ..
    } = no_data[0]
    else {
        unreachable!()
    };
    assert_eq!(viewer::strings::NO_DATA, text);
    assert!(*right <= rect.right());
}

#[test]
fn a_name_band_runs_under_the_name_slot_in_the_box_stroke() {
    let s = scene("[{:id :a :name \"Plain\" :crap {:mu 2} :killed 9 :survived 1}]");
    let commands = commands_of(&s);
    let bands = of_kind(&commands, "band");
    assert_eq!(1, bands.len());
    let Command::Band {
        rect, x, w, color, ..
    } = bands[0]
    else {
        unreachable!()
    };
    assert_eq!(rect.x + 6.0, *x);
    assert_eq!(rect.w - 12.0, *w);
    let g = scene::metric_rating(Some(2.0), Some(9.0), Some(1.0), None).grade;
    assert_eq!(viewer::theme::stroke(g), *color);
}

#[test]
fn edges_choose_heads_and_violation_widths_like_the_lua_adapter() {
    let text = "{:packages [{:id :p :label \"P\" :classes [{:id :a :name \"A\"} {:id :b :name \"B\"}]}] \
                :edges [{:from :a :to :b :kind :association} {:from :a :to :b :kind :dependency} \
                        {:from :a :to :b :kind :aggregation} {:from :a :to :b :kind :composition} \
                        {:from :a :to :b :kind :inheritance} {:from :a :to :b :kind :implements :violating true}]}";
    let s = compile_view(
        &scene::ir::read_document(text).unwrap(),
        &[],
        &[],
        &Value::Map(vec![]),
    );
    let heads: Vec<String> = commands::edge_commands(&s, None, None)
        .into_iter()
        .filter_map(|c| match c {
            EdgeCommand::Head { head, .. } => Some(head),
            _ => None,
        })
        .collect();
    assert_eq!(
        vec![
            "open",
            "open",
            "diamond",
            "diamond-fill",
            "triangle",
            "triangle"
        ],
        heads
    );
    // A hovered edge is hot; a violating one keeps its hot violation width.
    let hovered = edn::parse("{:kind :edge :from :d0-a :to :d0-b}").unwrap();
    let hot: Vec<(f32, viewer::theme::Rgb)> = commands::edge_commands(&s, None, Some(&hovered))
        .into_iter()
        .filter_map(|c| match c {
            EdgeCommand::Head { width, color, .. } => Some((width, color)),
            _ => None,
        })
        .collect();
    assert_eq!(6, hot.len());
    assert_eq!(
        (viewer::theme::widths::EDGE_HOT, viewer::theme::EDGE_HOT),
        hot[0]
    );
    assert_eq!(
        (
            viewer::theme::widths::EDGE_VIOLATING_HOT,
            viewer::theme::EDGE_VIOLATING_HOT
        ),
        *hot.last().unwrap()
    );
}

#[test]
fn hidden_edges_become_violation_aware_indicators() {
    let s = two_modules(
        "[{:from :a :to :b :kind :dependency :violating true}]",
        "{:declutter :triangles}",
    );
    let commands = commands::edge_commands(&s, None, None);
    assert_eq!(2, commands.len());
    for command in &commands {
        let EdgeCommand::Indicator {
            violating, points, ..
        } = command
        else {
            panic!("expected an indicator, got {command:?}")
        };
        assert!(violating);
        assert_eq!(3, points.len());
    }
}

#[test]
fn hide_edges_draws_indicators_for_any_lua_truthy_value() {
    // adapters/edges.lua: `if g(g(scene,'diagram'),'hide-edges') then` draw
    // indicators, else strokes. `:yes` hides; an authored `false` draws edges.
    // The triangles declutter computes the indicators; the scene's diagram
    // then carries the authored flag value.
    let commands_for = |hide: &str| {
        let mut s = two_modules(
            "[{:from :a :to :b :kind :dependency}]",
            "{:declutter :triangles}",
        );
        let mut diagram = edn::record::get(&s, "diagram").cloned().unwrap();
        edn::record::put(&mut diagram, "hide-edges", edn::parse(hide).unwrap());
        edn::record::put(&mut s, "diagram", diagram);
        commands::edge_commands(&s, None, None)
    };
    let yes = commands_for(":yes");
    assert_eq!(2, yes.len());
    assert!(
        yes.iter()
            .all(|c| matches!(c, EdgeCommand::Indicator { .. })),
        "{yes:?}"
    );
    let shown = commands_for("false");
    assert!(
        shown
            .iter()
            .any(|c| matches!(c, EdgeCommand::Stroke { .. })),
        "{shown:?}"
    );
    assert!(
        !shown
            .iter()
            .any(|c| matches!(c, EdgeCommand::Indicator { .. })),
        "{shown:?}"
    );
}

#[test]
fn indicator_hover_offers_the_from_to_dependency_rows() {
    // The chrome popup lists `from -> to` sorted with violations first.
    let s = edn::parse("{:classes [] :packages [] :edges [] :dep-indicators []}").unwrap();
    let hover = edn::parse(
        "{:kind :dep :id :a :deps [{:from :a :to :b :violating false} {:from :c :to :d :violating true}]}",
    )
    .unwrap();
    let mut state = scene::document::ViewState {
        scene: s,
        hover: Some(hover),
        pointer: Some((100.0, 100.0)),
        ..Default::default()
    };
    state.focus = vec![];
    let chrome = commands::chrome(&state, 800.0, 600.0);
    let popup = chrome
        .iter()
        .find_map(|c| match c {
            Command::DependencyPopup { rows, .. } => Some(rows),
            _ => None,
        })
        .expect("a dep hover with deps shows the popup");
    let rows: Vec<(String, bool)> = popup
        .iter()
        .map(|r| (r.text.clone(), r.violating))
        .collect();
    assert_eq!(
        vec![("c -> d".to_string(), true), ("a -> b".to_string(), false)],
        rows
    );
}

#[test]
fn waiting_state_chrome_shows_only_the_status_line() {
    let mut state = scene::document::waiting_state(Some("diagram.edn".into()), 0);
    let chrome = commands::chrome(&state, 800.0, 600.0);
    assert!(
        chrome
            .iter()
            .all(|c| matches!(c, Command::Status { .. } | Command::Hint { .. }))
    );
    state.hover = Some(edn::parse("{:kind :dep :deps [{:from :a :to :b}]}").unwrap());
    state.pointer = Some((1.0, 1.0));
    let chrome = commands::chrome(&state, 800.0, 600.0);
    assert!(
        !chrome
            .iter()
            .any(|c| matches!(c, Command::DependencyPopup { .. }))
    );
}

#[test]
fn the_fit_rule_never_grows_past_the_nominal_size() {
    // Monospace estimate like the busted fake font: text_w/16*size.
    let measure = |size: f64, text: &str| scene::text_w(Some(text)) / 16.0 * size;
    assert_eq!(18.0, commands::fit(measure, 18.0, "短", 200.0, true, false));
    let shrunk = commands::fit(
        measure,
        18.0,
        "AVeryLongClassNameThatFillsTheBox",
        100.0,
        true,
        false,
    );
    assert!(shrunk < 18.0);
    // The shrunk text plus its bold offset fits the budget.
    let natural = measure(shrunk, "AVeryLongClassNameThatFillsTheBox") + shrunk * 0.05;
    assert!(natural <= 100.0 + 1e-9);
}

#[test]
fn the_fit_rule_holds_for_a_measure_that_snaps_to_the_pixel_grid() {
    // Real fonts round advances, so width is not linear in size; the fitted
    // size must still measure within the budget (else egui wraps the label).
    let measure = |size: f64, text: &str| (text.chars().count() as f64 * size * 0.61).ceil();
    let text = "Detail_view";
    let fitted = commands::fit(measure, 18.0, text, 76.0, false, true);
    assert!(fitted < 18.0);
    assert!(
        measure(fitted, text) + fitted * 0.2 <= 76.0 + 1e-9,
        "fitted {fitted} still measures {}",
        measure(fitted, text) + fitted * 0.2
    );
}

#[test]
fn the_canvas_chrome_has_no_declutter_control() {
    // #141 moved Declutter into the inspector sidebar (its labels are
    // checked by the inspector tests); the canvas keeps only banner,
    // return scope and popups.
    let doc =
        scene::ir::read_document("{:hierarchical true :classes [{:id :a.b} {:id :a.c}] :edges []}")
            .unwrap();
    let state = scene::document::new_state(doc, None, None, None, &[], None);
    let kinds: Vec<&str> = commands::chrome(&state, 1500.0, 920.0)
        .iter()
        .map(|c| c.kind())
        .collect();
    assert!(!kinds.contains(&"declutter"), "{kinds:?}");
}

#[test]
fn the_glyph_probe_covers_the_whole_ui_string_table() {
    let probe = viewer::strings::glyph_text();
    for text in [
        "搜索类名或命名空间",
        "没有匹配的类",
        "精简：箭头",
        "源码图",
        "«»αμσ←→—…",
        "重新生成 IR",
    ] {
        assert!(probe.contains(text), "glyph probe lacks {text}");
    }
}

// #154: edges.lua and class_boxes.lua read these as `if g(x,'k')` (or
// `not not`), so an authored `false`/`nil` draws like omission and `:yes`/`1`
// like `true`. The omitted and `true` outputs are the reference.

const FALSY: [&str; 2] = ["false", "nil"];
const TRUTHY: [&str; 2] = [":yes", "1"];

#[test]
fn edge_and_indicator_violating_is_a_lua_truthy_flag() {
    // edges.lua:32 `local violating=g(e,'violating')`; edges.lua:24/53 the
    // indicator's `violating` picks the red band.
    let stroke = |flag: &str| {
        let s = edn::parse(&format!(
            "{{:classes [] :edges [{{:from :a :to :b :strokes [[[0 0] [10 10]]] {flag}}}]}}"
        ))
        .unwrap();
        commands::edge_commands(&s, None, None)
    };
    let indicator = |flag: &str| {
        let s = edn::parse(&format!(
            "{{:diagram {{:hide-edges true}} :classes [] \
             :dep-indicators [{{:id :a :triangle [[0 0] [6 0] [3 5]] {flag}}}]}}"
        ))
        .unwrap();
        commands::edge_commands(&s, None, None)
    };
    type Draw<'a> = &'a dyn Fn(&str) -> Vec<EdgeCommand>;
    for draw in [&stroke as Draw, &indicator] {
        let omitted = draw("");
        let violating = draw(":violating true");
        assert_ne!(omitted, violating, "fixture must tell a violation apart");
        for v in FALSY {
            assert_eq!(omitted, draw(&format!(":violating {v}")), "{v}");
        }
        for v in TRUTHY {
            assert_eq!(violating, draw(&format!(":violating {v}")), "{v}");
        }
    }
}

#[test]
fn dependency_popup_rows_read_violating_as_a_lua_truthy_flag() {
    // class_boxes.lua:179 `violating=not not G.get(d,'violating')`.
    for (v, want) in [
        ("false", false),
        ("nil", false),
        ("true", true),
        (":yes", true),
        ("1", true),
    ] {
        let hover = format!("{{:kind :dep :id :a :deps [{{:from :a :to :b :violating {v}}}]}}");
        let state = scene::document::ViewState {
            scene: edn::parse("{:classes [] :packages [] :edges [] :dep-indicators []}").unwrap(),
            hover: Some(edn::parse(&hover).unwrap()),
            pointer: Some((100.0, 100.0)),
            ..Default::default()
        };
        let rows: Vec<bool> = commands::chrome(&state, 800.0, 600.0)
            .iter()
            .find_map(|c| match c {
                Command::DependencyPopup { rows, .. } => {
                    Some(rows.iter().map(|r| r.violating).collect())
                }
                _ => None,
            })
            .expect("popup");
        assert_eq!(vec![want], rows, "{v}");
    }
}

#[test]
fn proposal_banner_reads_the_diagram_proposal_as_a_lua_truthy_flag() {
    // class_boxes.lua:162 `if G.get(diagram,'proposal') then` banner.
    let banner = |flag: &str| {
        let state = scene::document::ViewState {
            scene: edn::parse(&format!(
                "{{:diagram {{:title \"T\" {flag}}} :classes [] :packages [] :edges []}}"
            ))
            .unwrap(),
            ..Default::default()
        };
        commands::chrome(&state, 800.0, 600.0)
            .iter()
            .any(|c| matches!(c, Command::ProposalBanner { .. }))
    };
    assert!(!banner(""));
    assert!(banner(":proposal true"));
    for v in FALSY {
        assert!(!banner(&format!(":proposal {v}")), "{v}");
    }
    for v in TRUTHY {
        assert!(banner(&format!(":proposal {v}")), "{v}");
    }
}
