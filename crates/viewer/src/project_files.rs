//! Project files the inspector writes (#141): proposal write-back through
//! the scene [`Boundary`]. The `to-agent.edn` mail queue is written through
//! [`crate::mailbox::Mailbox`], the one owner of the queue contract.

use std::path::Path;

use scene::document::Boundary;

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
