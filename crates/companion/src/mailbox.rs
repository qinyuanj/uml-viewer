//! File IO for the mail queues (port of `love/uml/adapters/mailbox.lua`).
//! Atomic replace is a durability boundary, not a concurrent-writer lock; the
//! queue order on disk is authoritative.

use crate::posix::{self, Result};
use crate::record;
use edn::Value;

/// Normalize whatever is on disk into `{:next-id n :queue [...]}` (Lua
/// `Protocol.normalize`): a queue vector is kept, a bare command becomes a
/// one-element queue, anything else is empty.
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

pub struct Mailbox {
    path: std::path::PathBuf,
}

impl Mailbox {
    pub fn open(path: impl Into<std::path::PathBuf>) -> Mailbox {
        Mailbox { path: path.into() }
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

    fn temp() -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "uml mailbox {} {}",
            std::process::id(),
            posix::monotonic_ms()
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
}
