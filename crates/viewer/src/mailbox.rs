//! Mailbox (#142): the `to-viewer.edn` inbox, the `to-agent.edn` outbox and
//! the element menu contract of the LOVE viewer (`adapters/mailbox.lua`,
//! `adapters/mail.lua`, `adapters/mail_menu.lua`). The durable queue is
//! authoritative; a command is popped only after it was applied.

use std::path::{Path, PathBuf};

use edn::Value;
use edn::record::{get, local_name};
use scene::document::ViewState;
use scene::events;

use crate::project_files;

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

/// One `{:next-id n :queue [...]}` file. Reads normalize whatever is on
/// disk; `take` pops the head in stored order and rewrites the file.
pub struct Mailbox {
    pub path: PathBuf,
}

impl Mailbox {
    pub fn open(path: impl Into<PathBuf>) -> Mailbox {
        Mailbox { path: path.into() }
    }

    pub fn inbox(root: &Path) -> Mailbox {
        Mailbox::open(root.join(".uml-viewer/to-viewer.edn"))
    }

    pub fn outbox(root: &Path) -> Mailbox {
        Mailbox::open(root.join(".uml-viewer/to-agent.edn"))
    }

    pub fn read(&self) -> Value {
        let raw = std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|text| edn::parse(&text).ok());
        normalize(raw.as_ref())
    }

    fn write(&self, boxed: &Value) -> std::io::Result<()> {
        let text = edn::encode(boxed).map_err(|e| std::io::Error::other(e.to_string()))?;
        project_files::write_atomic(&self.path, &text)
    }

    /// Pop the head of the queue, whatever its id (the stored order is
    /// authoritative). `None` leaves the file untouched.
    pub fn take(&self) -> std::io::Result<Option<Value>> {
        let (head, rest) = take(&self.read());
        if head.is_some() {
            self.write(&rest)?;
        }
        Ok(head)
    }

    /// Append a command and return it (`{:id n :op …extra}`).
    pub fn append(&self, op: &str, extra: &[(Value, Value)]) -> std::io::Result<Value> {
        let current = self.read();
        let mut queue = queue(&current);
        let id = get(&current, "next-id")
            .and_then(edn::record::num)
            .unwrap_or(1.0) as i64;
        let mut cmd = Value::Map(vec![
            (Value::keyword("id"), Value::Integer(id)),
            (Value::keyword("op"), Value::keyword(op)),
        ]);
        for (k, v) in extra {
            cmd.assoc(k.clone(), v.clone());
        }
        queue.push(cmd.clone());
        let boxed = box_of(id + 1, queue);
        self.write(&boxed)?;
        Ok(cmd)
    }
}

fn box_of(next_id: i64, queue: Vec<Value>) -> Value {
    Value::Map(vec![
        (Value::keyword("next-id"), Value::Integer(next_id)),
        (Value::keyword("queue"), Value::Vector(queue)),
    ])
}

/// Normalize whatever is on disk into `{:next-id n :queue [...]}`: a queue
/// vector is kept, a bare command becomes a one-element queue, anything else
/// is empty.
pub fn normalize(raw: Option<&Value>) -> Value {
    if let Some(raw) = raw {
        if let Some(Value::Vector(items)) = get(raw, "queue") {
            let next = get(raw, "next-id")
                .and_then(edn::record::num)
                .unwrap_or(1.0) as i64;
            return box_of(next, items.clone());
        }
        if get(raw, "op").is_some() {
            let id = get(raw, "id").and_then(edn::record::num).unwrap_or(0.0) as i64;
            return box_of(id + 1, vec![raw.clone()]);
        }
    }
    box_of(1, Vec::new())
}

pub fn queue(queued: &Value) -> Vec<Value> {
    match get(queued, "queue") {
        Some(Value::Vector(items)) => items.clone(),
        _ => Vec::new(),
    }
}

/// Pop the head; the remainder keeps `:next-id`.
pub fn take(queued: &Value) -> (Option<Value>, Value) {
    let mut items = queue(queued);
    if items.is_empty() {
        return (None, queued.clone());
    }
    let head = items.remove(0);
    let next = get(queued, "next-id")
        .and_then(edn::record::num)
        .unwrap_or(1.0) as i64;
    (Some(head), box_of(next, items))
}

/// `mail.last_id`: the id of the last queued command (0 when empty). The
/// viewer captures it before a companion exists, so a `:display` written
/// during startup is consumed rather than skipped.
pub fn last_id(queued: &Value) -> i64 {
    queue(queued)
        .last()
        .and_then(|cmd| get(cmd, "id"))
        .and_then(edn::record::num)
        .unwrap_or(0.0) as i64
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
