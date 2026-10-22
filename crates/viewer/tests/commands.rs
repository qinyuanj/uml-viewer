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
    assert_eq!(Some(scene::config::GRADE_WORST), *grade);
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
    let g = scene::config::combined_grade(
        Some(scene::config::crap_grade(Some(2.0))),
        Some(scene::config::mutation_grade(Some(0.9))),
    );
    assert_eq!(viewer::theme::stroke(g), *color);
}

#[test]
fn edges_choose_heads_and_violation_widths_like_the_lua_adapter() {
    let text = "{:packages [{:id :p :label \"P\" :classes [{:id :a :name \"A\"} {:id :b :name \"B\"}]}] \
                :edges [{:from :a :to :b :kind :association} {:from :a :to :b :kind :dependency} \
                        {:from :a :to :b :kind :aggregation} {:from :a :to :b :kind :composition} \
                        {:from :a :to :b :kind :inheritance} {:from :a :to :b :kind :implements :violating true}]}";
    let s = scene::compose::compile_diagram(&edn::parse(text).unwrap());
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
    let hovered = edn::parse("{:kind :edge :from :a :to :b}").unwrap();
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
    let text = "{:hide-edges true :packages [{:id :p :label \"P\" :classes [{:id :a :name \"A\"} {:id :b :name \"B\"}]}] \
                :edges [{:from :a :to :b :kind :dependency :violating true}]}";
    let s = scene::compose::compile_diagram(&edn::parse(text).unwrap());
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
    let measure = |size: f64, text: &str| scene::layout::text_w(Some(text)) / 16.0 * size;
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
