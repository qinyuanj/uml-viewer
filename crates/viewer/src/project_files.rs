//! Project files the inspector writes (#141): proposal write-back through
//! the scene [`Boundary`] and the Regen request appended to
//! `.uml-viewer/to-agent.edn`. Writes replace atomically (temp + rename);
//! the queue shape is the `{:next-id n :queue [cmd …]}` mail contract.
//! Companion wake-up and the mail reader belong to #142.

use std::path::Path;

use edn::Value;
use scene::document::Boundary;
use scene::geom::{get, num};

/// Write `text` next to `path` and rename it into place.
pub fn write_atomic(path: &Path, text: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension(format!("tmp-{}", std::process::id()));
    std::fs::write(&tmp, text)?;
    std::fs::rename(&tmp, path)
}

/// The filesystem as the proposal write-back boundary.
pub struct Files;

impl Boundary for Files {
    fn exists(&self, path: &str) -> bool {
        Path::new(path).is_file()
    }

    fn read(&self, path: &str) -> Option<String> {
        std::fs::read_to_string(path).ok()
    }

    fn write(&self, path: &str, text: &str) -> bool {
        match write_atomic(Path::new(path), text) {
            Ok(()) => true,
            Err(error) => {
                eprintln!("uml viewer: could not write {path}: {error}");
                false
            }
        }
    }
}

/// Append `{:id n :op op …extra}` to a mail queue file; returns the command.
pub fn append(path: &Path, op: &str, extra: &[(Value, Value)]) -> std::io::Result<Value> {
    let current = std::fs::read_to_string(path)
        .ok()
        .and_then(|t| edn::parse(&t).ok())
        .unwrap_or(Value::Nil);
    let mut queue = match get(&current, "queue") {
        Some(Value::Vector(q)) => q.clone(),
        _ => Vec::new(),
    };
    let id = get(&current, "next-id").and_then(num).unwrap_or(1.0) as i64;
    let mut cmd = Value::Map(vec![
        (Value::keyword("id"), Value::Integer(id)),
        (Value::keyword("op"), Value::keyword(op)),
    ]);
    for (k, v) in extra {
        cmd.assoc(k.clone(), v.clone());
    }
    queue.push(cmd.clone());
    let boxed = Value::Map(vec![
        (Value::keyword("next-id"), Value::Integer(id + 1)),
        (Value::keyword("queue"), Value::Vector(queue)),
    ]);
    let text = edn::encode(&boxed).map_err(|e| std::io::Error::other(e.to_string()))?;
    write_atomic(path, &text)?;
    Ok(cmd)
}
