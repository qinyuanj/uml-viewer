//! The element menu contract of the LOVE viewer (`adapters/mail_menu.lua`,
//! #142): the menu operations, the mail target of a selection and the mail
//! each operation appends. The queue protocol itself is
//! [`companion::mailbox::Mailbox`].

use edn::Value;
use edn::record::{get, local_name};
use scene::document::ViewState;
use scene::events;

/// The section prefix of a scene id (`d0-target` -> 0), or 0.
fn section(id: &Value) -> usize {
    local_name(id)
        .unwrap_or_default()
        .to_owned()
        .strip_prefix('d')
        .and_then(|rest| rest.split_once('-'))
        .and_then(|(n, _)| n.parse().ok())
        .unwrap_or(0)
}

/// The element menu operations, in the order the menu shows them. The labels
/// live in `strings::MENU`; the mail operations stay English.
pub const MENU_OPS: [&str; 4] = [
    "refresh-crap",
    "refresh-mutate",
    "refresh-mutate-all",
    "omit",
];

/// `mail_menu.target`: the mail target of the current selection. A proposal
/// layer, a dummy class, a class with nested contents and a package are
/// components; the namespace and the proposal id travel with the target.
pub fn target(state: &ViewState, selection: Option<&Value>) -> Value {
    let selected = selection.and_then(|s| get(s, "id")).cloned();
    let class = selected.as_ref().and_then(|id| find_class(state, id));
    // Name the authored IR class: the scene prefixes section ids (`d0-x`),
    // and the mail contract never carried that prefix.
    let id = class.and_then(|c| get(c, "id")).cloned().or(selected);
    let component = events::layer_id(selection).is_some()
        || class.is_some_and(|c| edn::record::flag(c, "dummy?"))
        || class.is_some_and(
            |c| matches!(get(c, "contents"), Some(Value::Vector(items)) if !items.is_empty()),
        )
        || selection.and_then(|s| get(s, "kind")).and_then(local_name) == Some("package");
    let mut target = Value::Map(vec![
        (
            Value::keyword("kind"),
            Value::keyword(if component { "component" } else { "class" }),
        ),
        (Value::keyword("id"), id.unwrap_or(Value::Nil)),
    ]);
    if let Some(ns) = class.and_then(|c| get(c, "ns")) {
        target.assoc(Value::keyword("ns"), ns.clone());
    }
    if let Some(pid) = get(&state.extra, "proposal-id").filter(|v| **v != Value::Nil) {
        target.assoc(Value::keyword("proposal-id"), pid.clone());
    }
    target
}

/// The document's class record for a selection id (the IR, not the compiled
/// scene). A scene id also matches the authored id it was prefixed from.
fn find_class<'a>(state: &'a ViewState, id: &Value) -> Option<&'a Value> {
    let doc = state.document.as_ref()?;
    fn walk<'a>(items: Option<&'a Value>, id: &Value) -> Option<&'a Value> {
        let Some(Value::Vector(items)) = items else {
            return None;
        };
        let prefix = format!("d{}-", section(id));
        let authored = local_name(id).unwrap_or_default().to_owned();
        let authored = authored
            .strip_prefix(&prefix)
            .unwrap_or(&authored)
            .to_owned();
        let matches = |candidate: Option<&Value>| {
            candidate.is_some_and(|candidate| {
                candidate == id
                    || local_name(candidate).unwrap_or_default() == authored
                    || local_name(id).unwrap_or_default().ends_with(&format!(
                        "-{}",
                        local_name(candidate).unwrap_or_default().to_owned()
                    ))
            })
        };
        for item in items {
            if matches(get(item, "id")) {
                return Some(item);
            }
            // Classes nest under packages and diagrams; diagrams may also
            // carry their own packages.
            for key in ["classes", "packages", "diagrams"] {
                if let Some(found) = walk(get(item, key), id) {
                    return Some(found);
                }
            }
        }
        None
    }
    ["classes", "packages", "diagrams"]
        .iter()
        .find_map(|key| walk(get(doc, key), id))
}

/// One menu action: the operation's mail with `:target` (the exact
/// `to-agent.edn` contract of the LOVE viewer).
pub fn menu_mail(
    state: &ViewState,
    selection: Option<&Value>,
    op: &str,
) -> (String, Vec<(Value, Value)>) {
    (
        op.to_string(),
        vec![(Value::keyword("target"), target(state, selection))],
    )
}
