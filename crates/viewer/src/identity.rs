//! Rust viewer process identity (port of
//! `love/uml/adapters/process_identity.lua`): PID + starttime self-report in
//! the exact JSON shape the restart wrapper compares. The runtime field is
//! `rust` here; `uml_launch.verify_previous` accepts both.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const FILE_NAME: &str = "viewer-process-identity.json";

/// starttime of a live process: /proc/<pid>/stat field 22, which is index 19
/// of the fields after the closing parenthesis (comm in parens may contain
/// spaces).
pub fn starttime(pid: u32) -> Option<String> {
    let text = fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let fields: Vec<&str> = text.rsplit(')').next()?.split_whitespace().collect();
    fields.get(19).map(|s| s.to_string())
}

/// `{"pid":1234,"started":"5381724","runtime":"rust","cwd":"/abs/root"}` —
/// fixed key order, no spaces, no trailing newline.
pub fn encode(pid: u32, started: &str, cwd: &str) -> String {
    format!("{{\"pid\":{pid},\"started\":\"{started}\",\"runtime\":\"rust\",\"cwd\":\"{cwd}\"}}")
}

fn write_atomic(target: &Path, text: &str) -> io::Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    let temp = target.with_extension("json.tmp");
    fs::write(&temp, text)?;
    fs::rename(&temp, target)
}

/// Write the identity document for the running process. Only called after a
/// successful startup; a refused startup never overwrites an existing file.
pub fn record(root: &Path) -> io::Result<PathBuf> {
    let pid = std::process::id();
    let started = starttime(pid)
        .ok_or_else(|| io::Error::other("Could not read the viewer creation identity"))?;
    let cwd = root.to_string_lossy();
    let target = root.join(".uml-viewer").join(FILE_NAME);
    write_atomic(&target, &encode(pid, &started, &cwd))?;
    Ok(target)
}
