//! Ctrl+F search box state (port of the search part of
//! `love/uml/adapters/inspector.lua`): explicit Down/Up selection, reset on
//! a new document, and the screen area that captures canvas input.

use edn::Value;
use scene::geom::Rect;
use viewer::search_box::SearchBox;

fn doc() -> Value {
    edn::parse(
        r#"{:hierarchical true :classes [{:id :alpha.deep.parser :name "解析器" :ns "love.alpha.deep.parser"}
            {:id :beta.parser :name "解析器" :ns "love.beta.parser"}
            {:id :gamma :name "Other" :ns "love.gamma"}] :edges []}"#,
    )
    .unwrap()
}

#[test]
fn down_and_up_select_explicitly_and_clamp_at_the_ends() {
    let doc = doc();
    let mut search = SearchBox::default();
    search.open();
    assert!(search.active && search.focus_request);
    search.query = "解析".into();
    let results = search.results(Some(&doc));
    let ids: Vec<String> = results
        .iter()
        .map(|m| {
            edn::record::local_name(&m.id)
                .unwrap_or_default()
                .to_owned()
        })
        .collect();
    assert_eq!(vec!["alpha.deep.parser", "beta.parser"], ids);
    // Nothing is selected until Down is pressed.
    assert_eq!(None, search.selection);
    search.step(true, &results);
    assert_eq!(Some(Value::keyword("alpha.deep.parser")), search.selection);
    search.step(true, &results);
    search.step(true, &results);
    assert_eq!(Some(Value::keyword("beta.parser")), search.selection);
    search.step(false, &results);
    search.step(false, &results);
    assert_eq!(Some(Value::keyword("alpha.deep.parser")), search.selection);
    // Up with no selection picks the first row (Lua: (index or 2)-1).
    search.selection = None;
    search.step(false, &results);
    assert_eq!(Some(Value::keyword("alpha.deep.parser")), search.selection);
    // ASCII is case-insensitive.
    search.query = "OTHER".into();
    assert_eq!(1, search.results(Some(&doc)).len());
}

#[test]
fn a_new_document_revision_forgets_the_selection() {
    let mut search = SearchBox::default();
    search.sync(1);
    search.selection = Some(Value::keyword("alpha.deep.parser"));
    search.sync(1);
    assert!(search.selection.is_some());
    search.sync(2);
    assert_eq!(None, search.selection);
}

#[test]
fn the_open_box_captures_pointer_input_inside_its_area_only() {
    let mut search = SearchBox::default();
    search.area = Some(Rect::new(500.0, 40.0, 440.0, 120.0));
    search.rows = vec![(
        Rect::new(510.0, 80.0, 420.0, 24.0),
        Value::keyword("beta.parser"),
    )];
    assert!(
        !search.captures(600.0, 60.0),
        "a closed box captures nothing"
    );
    search.open();
    assert!(search.captures(600.0, 60.0));
    assert!(!search.captures(100.0, 600.0));
    assert_eq!(
        Some(Value::keyword("beta.parser")),
        search.row_at(600.0, 90.0)
    );
    assert_eq!(None, search.row_at(600.0, 60.0));
    let record = search.record(&[]);
    assert!(record.active);
    search.close();
    assert!(!search.active && !search.captures(600.0, 60.0));
}
