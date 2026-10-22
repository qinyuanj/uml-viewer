//! Companion ownership file `.uml-viewer/companion.edn` (port of
//! `love/uml/adapters/ownership.lua`). EDN on disk, atomically replaced.

use crate::posix::{self, Failure, Result};
use edn::Value;
use std::path::{Path, PathBuf};

pub fn path(root: &str) -> PathBuf {
    Path::new(root).join(".uml-viewer/companion.edn")
}

/// `Ok(None)` is an absent file; a corrupt file is an error, and its bytes
/// are retained.
pub fn read(root: &str) -> Result<Option<Value>> {
    let path = path(root);
    let Ok(text) = std::fs::read_to_string(&path) else {
        return Ok(None);
    };
    edn::parse(&text).map(Some).map_err(|_| {
        Failure::new("Present companion ownership is corrupt or unreadable; file retained")
    })
}

pub fn write(root: &str, record: &Value) -> Result<()> {
    let target = path(root);
    posix::atomic::ensure_parent(&target)?;
    posix::atomic::write(&target, edn::encode(record)?.as_bytes())
}

pub fn bytes(root: &str) -> Option<String> {
    std::fs::read_to_string(path(root)).ok()
}

pub fn exists(root: &str) -> bool {
    path(root).is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ownership_round_trips_as_stable_edn_and_refuses_corrupt_bytes() {
        let directory = std::env::temp_dir().join(format!(
            "uml ownership {} {}",
            std::process::id(),
            posix::monotonic_ms()
        ));
        let root = directory.to_string_lossy().into_owned();
        assert_eq!(None, read(&root).unwrap());
        let record = edn::parse(r#"{:backend :herdr :session "uml-viewer-A"}"#).unwrap();
        write(&root, &record).unwrap();
        assert_eq!(record, read(&root).unwrap().unwrap());
        let first = bytes(&root).unwrap();
        write(&root, &record).unwrap();
        assert_eq!(first, bytes(&root).unwrap(), "encoding must be byte-stable");
        std::fs::write(path(&root), "{:backend :herdr").unwrap();
        let error = read(&root).unwrap_err();
        assert!(error.message.contains("corrupt or unreadable"));
        assert_eq!(
            "{:backend :herdr",
            bytes(&root).unwrap(),
            "bytes are retained"
        );
        let _ = std::fs::remove_dir_all(&directory);
    }
}
