//! The mail queue protocol (port of `love/uml/adapters/mailbox.lua`): the one
//! owner of `{:next-id n :queue [cmd …]}` for the viewer's inbox and outbox
//! and the companion worker. Atomic replace is a durability boundary, not a
//! concurrent-writer lock; the queue order on disk is authoritative.

use std::path::{Path, PathBuf};

use crate::posix;
use crate::record;
use crate::vocab::{Failure, Result};
use edn::Value;

/// Normalize whatever is on disk into `{:next-id n :queue [...]}` (Lua
/// `Protocol.normalize`): a queue vector is kept with `:next-id` as stored, a
/// bare command becomes a one-element queue numbered after its integer id,
/// anything else is empty. An integer reading is an integer or a whole float.
pub fn normalize(raw: Option<&Value>) -> Value {
    if let Some(raw) = raw {
        if let Some(Value::Vector(queue)) = edn::record::get(raw, "queue") {
            let next = edn::record::get(raw, "next-id")
                .cloned()
                .unwrap_or(Value::Integer(1));
            return edn::record::record(vec![
                ("next-id", next),
                ("queue", Value::Vector(queue.clone())),
            ]);
        }
        if edn::record::get(raw, "op").is_some() {
            let id = record::get_i64(raw, "id").unwrap_or(0);
            return edn::record::record(vec![
                ("next-id", Value::Integer(id + 1)),
                ("queue", Value::Vector(vec![raw.clone()])),
            ]);
        }
    }
    edn::record::record(vec![
        ("next-id", Value::Integer(1)),
        ("queue", Value::Vector(vec![])),
    ])
}

pub fn queue(boxed: &Value) -> Vec<Value> {
    edn::record::seq(edn::record::get(boxed, "queue")).to_vec()
}

/// Pop the head of `:queue` whenever one exists, whatever its id. The stored
/// order is authoritative, never id sorting.
pub fn take(boxed: &Value) -> (Option<Value>, Value) {
    let mut items = queue(boxed);
    if items.is_empty() {
        return (None, boxed.clone());
    }
    let head = items.remove(0);
    let next = edn::record::get(boxed, "next-id")
        .cloned()
        .unwrap_or(Value::Integer(1));
    (
        Some(head),
        edn::record::record(vec![("next-id", next), ("queue", Value::Vector(items))]),
    )
}

/// The integer id of the last queued command (0 when empty). The viewer
/// captures it before a companion exists, so a `:display` written during
/// startup is consumed rather than skipped.
pub fn last_id(boxed: &Value) -> i64 {
    queue(boxed)
        .last()
        .and_then(|cmd| record::get_i64(cmd, "id"))
        .unwrap_or(0)
}

pub struct Mailbox {
    path: PathBuf,
}

impl Mailbox {
    pub fn open(path: impl Into<PathBuf>) -> Mailbox {
        Mailbox { path: path.into() }
    }

    /// `.uml-viewer/to-viewer.edn`: mail the viewer pops.
    pub fn inbox(project_root: &Path) -> Mailbox {
        Mailbox::open(project_root.join(".uml-viewer/to-viewer.edn"))
    }

    /// `.uml-viewer/to-agent.edn`: mail the viewer appends for the agent.
    pub fn outbox(project_root: &Path) -> Mailbox {
        Mailbox::open(project_root.join(".uml-viewer/to-agent.edn"))
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn read(&self) -> Value {
        let raw = std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|text| edn::parse(&text).ok());
        normalize(raw.as_ref())
    }

    pub fn write(&self, boxed: &Value) -> Result<()> {
        posix::atomic::write(&self.path, edn::encode(boxed)?.as_bytes())
    }

    pub fn take(&self) -> Result<Option<Value>> {
        let (head, rest) = take(&self.read());
        if head.is_some() {
            self.write(&rest)?;
        }
        Ok(head)
    }

    /// Append `{:id n :op :<op> …extra}` numbered by `:next-id` and return
    /// it. A `:next-id` without an integer reading fails and leaves the file:
    /// a guessed id could repeat one and void the consumption proof.
    pub fn append(&self, op: &str, extra: &[(Value, Value)]) -> Result<Value> {
        let current = self.read();
        let id = record::get_i64(&current, "next-id").ok_or_else(|| {
            Failure::new(format!("{} has no integer :next-id", self.path.display()))
        })?;
        let mut cmd =
            edn::record::record(vec![("id", Value::Integer(id)), ("op", Value::keyword(op))]);
        for (key, value) in extra {
            cmd.assoc(key.clone(), value.clone());
        }
        let mut items = queue(&current);
        items.push(cmd.clone());
        self.write(&edn::record::record(vec![
            ("next-id", Value::Integer(id + 1)),
            ("queue", Value::Vector(items)),
        ]))?;
        Ok(cmd)
    }

    /// `true` when the delivery's id has left the queue (consumption proof).
    pub fn acknowledged(&self, id: i64) -> bool {
        !queue(&self.read())
            .iter()
            .any(|queued| record::get_i64(queued, "id") == Some(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp() -> PathBuf {
        static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        std::env::temp_dir().join(format!(
            "uml mailbox {} {} {}",
            std::process::id(),
            posix::monotonic_ms(),
            COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ))
    }

    #[test]
    fn normalizes_queues_bare_commands_and_garbage() {
        let queued = edn::parse(r#"{:next-id 3 :queue [{:id 2 :op :regen}]}"#).unwrap();
        assert_eq!(queued, normalize(Some(&queued)));
        let bare = edn::parse(r#"{:id 7 :op :display :path "d.edn"}"#).unwrap();
        let normalized = normalize(Some(&bare));
        assert_eq!(Some(8), record::get_i64(&normalized, "next-id"));
        assert_eq!(vec![bare], queue(&normalized));
        let empty = normalize(Some(&edn::parse("[:invalid]").unwrap()));
        assert_eq!(Some(1), record::get_i64(&empty, "next-id"));
        assert!(queue(&empty).is_empty());
        assert!(queue(&normalize(None)).is_empty());
    }

    #[test]
    fn take_pops_the_head_in_stored_order() {
        let boxed = edn::parse(
            r#"{:next-id 4 :queue [{:id 3 :op :display} {:id 1 :op :regen} {:id 2 :op :context}]}"#,
        )
        .unwrap();
        let (head, rest) = take(&boxed);
        assert_eq!(Some(3), record::get_i64(&head.unwrap(), "id"));
        let remaining: Vec<i64> = queue(&rest)
            .iter()
            .map(|cmd| record::get_i64(cmd, "id").unwrap())
            .collect();
        assert_eq!(vec![1, 2], remaining);
        assert_eq!(Some(4), record::get_i64(&rest, "next-id"));
        let (none, same) = take(&edn::record::record(vec![("next-id", Value::Integer(1))]));
        assert!(none.is_none());
        assert!(queue(&same).is_empty());
    }

    #[test]
    fn the_file_mailbox_acknowledges_only_when_the_id_left_the_queue() {
        let path = temp();
        let mailbox = Mailbox::open(&path);
        assert!(queue(&mailbox.read()).is_empty());
        mailbox
            .write(
                &edn::parse(r#"{:next-id 3 :queue [{:id 1 :op :regen} {:id 2 :op :display}]}"#)
                    .unwrap(),
            )
            .unwrap();
        assert!(!mailbox.acknowledged(1));
        assert_eq!(
            Some(1),
            record::get_i64(&mailbox.take().unwrap().unwrap(), "id")
        );
        assert!(mailbox.acknowledged(1));
        assert!(!mailbox.acknowledged(2));
        assert_eq!(
            Some(2),
            record::get_i64(&mailbox.take().unwrap().unwrap(), "id")
        );
        assert!(mailbox.take().unwrap().is_none());
        let _ = std::fs::remove_file(&path);
    }

    fn ids(boxed: &Value) -> Vec<i64> {
        queue(boxed)
            .iter()
            .map(|cmd| record::get_i64(cmd, "id").unwrap())
            .collect()
    }

    #[test]
    fn bare_command_ids_read_as_integers_or_not_at_all() {
        let whole = normalize(Some(&edn::parse("{:id 3.0 :op :regen}").unwrap()));
        assert_eq!(
            Some(&Value::Integer(4)),
            edn::record::get(&whole, "next-id")
        );
        let fractional = normalize(Some(&edn::parse("{:id 3.5 :op :regen}").unwrap()));
        assert_eq!(
            Some(&Value::Integer(1)),
            edn::record::get(&fractional, "next-id")
        );
        assert_eq!(1, queue(&fractional).len());
        let odd = edn::parse(r#"{:next-id "x" :queue [{:id 1 :op :regen}]}"#).unwrap();
        assert_eq!(odd, normalize(Some(&odd)));
        let (_, rest) = take(&odd);
        assert_eq!(
            Some(&Value::String("x".into())),
            edn::record::get(&rest, "next-id")
        );
    }

    #[test]
    fn last_id_is_the_tail_command_id_or_zero() {
        assert_eq!(0, last_id(&normalize(None)));
        let boxed =
            edn::parse("{:next-id 9 :queue [{:id 5 :op :regen} {:id 2.0 :op :regen}]}").unwrap();
        assert_eq!(2, last_id(&boxed));
    }

    #[test]
    fn append_numbers_commands_from_next_id_and_keeps_extra_keys() {
        let path = temp();
        let mailbox = Mailbox::open(&path);
        let target = edn::parse("{:kind :class :id :app.target}").unwrap();
        let first = mailbox
            .append("omit", &[(Value::keyword("target"), target.clone())])
            .unwrap();
        assert_eq!(
            edn::parse("{:id 1 :op :omit :target {:kind :class :id :app.target}}").unwrap(),
            first
        );
        mailbox.append("regen", &[]).unwrap();
        let boxed = mailbox.read();
        assert_eq!(vec![1, 2], ids(&boxed));
        assert_eq!(Some(3), record::get_i64(&boxed, "next-id"));
        assert_eq!(2, last_id(&boxed));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn append_accepts_a_whole_float_next_id_and_extends_a_bare_command() {
        let path = temp();
        let mailbox = Mailbox::open(&path);
        std::fs::write(&path, "{:next-id 3.0 :queue []}").unwrap();
        assert_eq!(
            Some(3),
            record::get_i64(&mailbox.append("regen", &[]).unwrap(), "id")
        );
        assert_eq!(
            Some(&Value::Integer(4)),
            edn::record::get(&mailbox.read(), "next-id")
        );
        std::fs::write(&path, "{:id 6 :op :omit}").unwrap();
        mailbox.append("regen", &[]).unwrap();
        assert_eq!(
            edn::parse("{:next-id 8 :queue [{:id 6 :op :omit} {:id 7 :op :regen}]}").unwrap(),
            mailbox.read()
        );
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn append_refuses_a_non_integer_next_id_and_leaves_the_file() {
        let path = temp();
        let mailbox = Mailbox::open(&path);
        for text in ["{:next-id 2.5 :queue []}", r#"{:next-id "2" :queue []}"#] {
            std::fs::write(&path, text).unwrap();
            assert!(mailbox.append("regen", &[]).is_err());
            assert_eq!(text, std::fs::read_to_string(&path).unwrap());
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn an_outbox_append_is_acknowledged_once_taken() {
        let root = temp();
        let outbox = Mailbox::outbox(&root);
        assert_eq!(root.join(".uml-viewer/to-agent.edn"), outbox.path());
        assert_eq!(
            root.join(".uml-viewer/to-viewer.edn"),
            Mailbox::inbox(&root).path()
        );
        let first = record::get_i64(&outbox.append("regen", &[]).unwrap(), "id").unwrap();
        let second = record::get_i64(&outbox.append("regen", &[]).unwrap(), "id").unwrap();
        assert!(!outbox.acknowledged(first));
        let taken = outbox.take().unwrap().unwrap();
        assert_eq!(Some(first), record::get_i64(&taken, "id"));
        assert!(outbox.acknowledged(first));
        assert!(!outbox.acknowledged(second));
        let _ = std::fs::remove_dir_all(&root);
    }
}
