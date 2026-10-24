//! Mailbox contract (#142): the `to-viewer.edn` inbox pops the head in stored
//! order whatever its id (a restored cursor never skips mail), the
//! `to-agent.edn` outbox appends `{:id n :op …}`, and the element menu writes
//! the exact target shape the LOVE viewer wrote.

use edn::Value;
use edn::record::get;
use scene::document;
use viewer::mailbox::{self, MENU_OPS, Mailbox};

const QUEUE: &str = "{:next-id 3 :queue [{:id 1 :op :context :context :proposal :proposal-id :p :name \"中文\"} {:id 2 :op :display :path \"second.edn\"}]}";

const DIAGRAM: &str = "{:title \"Second diagram\" :hierarchical true :prefix \"app\" :classes [{:id :app.target :name \"target\" :ns \"app.target\" :contents [{:id :inner}]}] :proposals [{:id :p :name \"P\" :layers []}]}";

fn project() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "uml mailbox {} {}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(dir.join(".uml-viewer")).unwrap();
    dir
}

#[test]
fn the_inbox_pops_the_head_in_stored_order_and_leaves_the_rest() {
    let dir = project();
    let path = dir.join(".uml-viewer/to-viewer.edn");
    std::fs::write(&path, QUEUE).unwrap();
    let inbox = Mailbox::inbox(&dir);
    let first = inbox.take().unwrap().unwrap();
    assert_eq!(Some(&Value::keyword("context")), get(&first, "op"));
    let second = inbox.take().unwrap().unwrap();
    assert_eq!(Some(&Value::keyword("display")), get(&second, "op"));
    assert!(inbox.take().unwrap().is_none());
    let rest = inbox.read();
    assert_eq!(Some(&Value::Integer(3)), get(&rest, "next-id"));
    assert!(mailbox::queue(&rest).is_empty());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_cursor_beyond_the_queued_ids_never_hides_mail() {
    let boxed = edn::parse("{:next-id 9 :queue [{:id 2 :op :regen}]}").unwrap();
    assert_eq!(2, mailbox::last_id(&boxed));
    let (head, rest) = mailbox::take(&boxed);
    let head = head.unwrap();
    assert_eq!(Some(2.0), get(&head, "id").and_then(edn::record::num));
    assert!(mailbox::queue(&rest).is_empty());
    assert_eq!(
        0,
        mailbox::last_id(&edn::parse("{:next-id 1 :queue []}").unwrap())
    );
}

#[test]
fn a_bare_command_and_garbage_normalize_like_the_love_protocol() {
    let bare = edn::parse("{:id 7 :op :display :path \"d.edn\"}").unwrap();
    let normalized = mailbox::normalize(Some(&bare));
    assert_eq!(Some(&Value::Integer(8)), get(&normalized, "next-id"));
    assert_eq!(vec![bare], mailbox::queue(&normalized));
    let garbage = mailbox::normalize(Some(&edn::parse("[:invalid]").unwrap()));
    assert_eq!(Some(&Value::Integer(1)), get(&garbage, "next-id"));
    assert!(mailbox::queue(&garbage).is_empty());
    assert!(mailbox::queue(&mailbox::normalize(None)).is_empty());
}

#[test]
fn the_outbox_appends_ids_from_next_id_and_keeps_earlier_commands() {
    let dir = project();
    let outbox = Mailbox::outbox(&dir);
    let target = Value::Map(vec![
        (Value::keyword("kind"), Value::keyword("class")),
        (Value::keyword("id"), Value::keyword("app.target")),
    ]);
    let extra = vec![(Value::keyword("target"), target)];
    for op in MENU_OPS {
        outbox.append(op, &extra).unwrap();
    }
    let text = std::fs::read_to_string(dir.join(".uml-viewer/to-agent.edn")).unwrap();
    let expected = "{:next-id 5 :queue [\
{:id 1 :op :refresh-crap :target {:id :app.target :kind :class}} \
{:id 2 :op :refresh-mutate :target {:id :app.target :kind :class}} \
{:id 3 :op :refresh-mutate-all :target {:id :app.target :kind :class}} \
{:id 4 :op :omit :target {:id :app.target :kind :class}}]}";
    assert_eq!(expected, text);
    let _ = std::fs::remove_dir_all(dir);
}

fn state() -> scene::document::ViewState {
    let doc = scene::ir::read_document(DIAGRAM).unwrap();
    let mut state = document::new_state(doc, None, None, None, &[], None);
    state
        .extra
        .assoc(Value::keyword("proposal-id"), Value::keyword("p"));
    state
}

#[test]
fn a_menu_target_carries_the_namespace_and_the_proposal_under_discussion() {
    let state = state();
    let selected = edn::parse("{:kind :class :id :app.target}").unwrap();
    let target = mailbox::target(&state, Some(&selected));
    assert_eq!(Some(&Value::keyword("component")), get(&target, "kind"));
    assert_eq!(Some(&Value::keyword("app.target")), get(&target, "id"));
    assert_eq!(
        Some(&Value::String("app.target".into())),
        get(&target, "ns")
    );
    assert_eq!(Some(&Value::keyword("p")), get(&target, "proposal-id"));
}

#[test]
fn a_leaf_class_without_contents_is_a_class_target() {
    let state = state();
    let selected = edn::parse("{:kind :class :id :app}").unwrap();
    let target = mailbox::target(&state, Some(&selected));
    assert_eq!(Some(&Value::keyword("class")), get(&target, "kind"));
    assert_eq!(
        None,
        get(&target, "ns"),
        "a class without a namespace carries none"
    );
    // A proposal row and a package are components.
    let package = edn::parse("{:kind :package :id :app.target}").unwrap();
    assert_eq!(
        Some(&Value::keyword("component")),
        get(&mailbox::target(&state, Some(&package)), "kind")
    );
    let proposal = edn::parse("{:kind :class :id :p :drill? true}").unwrap();
    assert_eq!(
        Some(&Value::keyword("component")),
        get(&mailbox::target(&state, Some(&proposal)), "kind")
    );
}

#[test]
fn a_scene_id_resolves_to_the_authored_document_class_and_its_namespace() {
    // The compiled scene prefixes section ids (`d0-target`); the mail
    // contract names the authored IR class, as the LOVE viewer did.
    let doc = scene::ir::read_document(
        "{:title \"Second diagram\" :packages [{:label \"P\" :classes [{:id :target :ns \"app.target\"}]}]}",
    )
    .unwrap();
    let state = document::new_state(doc, None, None, None, &[], None);
    let selected = edn::parse("{:kind :class :id :d0-target}").unwrap();
    let target = mailbox::target(&state, Some(&selected));
    assert_eq!(Some(&Value::keyword("target")), get(&target, "id"));
    assert_eq!(
        Some(&Value::String("app.target".into())),
        get(&target, "ns")
    );
    assert_eq!(Some(&Value::keyword("class")), get(&target, "kind"));
}

#[test]
fn a_dummy_class_is_a_component_target_for_any_lua_truthy_flag() {
    // `mail_menu.lua` `target`: `... or G.get(class,'dummy?') or ...`, a Lua
    // truthiness test on the document class. A hierarchical document is
    // read raw, so an authored `:dummy?` reaches the target.
    let kind = |fragment: &str| {
        let doc = scene::ir::read_document(&format!(
            "{{:title \"Dummy\" :hierarchical true :prefix \"app\" :classes [{{:id :app.stub :ns \"app.stub\" {fragment}}}]}}"
        ))
        .unwrap();
        let state = document::new_state(doc, None, None, None, &[], None);
        let selected = edn::parse("{:kind :class :id :app.stub}").unwrap();
        get(&mailbox::target(&state, Some(&selected)), "kind").cloned()
    };
    for falsy in ["", ":dummy? false", ":dummy? nil"] {
        assert_eq!(Some(Value::keyword("class")), kind(falsy), "{falsy}");
    }
    for truthy in [":dummy? true", ":dummy? :yes", ":dummy? 1"] {
        assert_eq!(Some(Value::keyword("component")), kind(truthy), "{truthy}");
    }
}

#[test]
fn menu_mail_names_the_operation_and_the_target() {
    let state = state();
    let selected = edn::parse("{:kind :class :id :app.target}").unwrap();
    let (op, extra) = mailbox::menu_mail(&state, Some(&selected), "omit");
    assert_eq!("omit", op);
    assert_eq!(1, extra.len());
    assert_eq!(Value::keyword("target"), extra[0].0);
}
