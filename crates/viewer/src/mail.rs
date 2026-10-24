//! Mail delivery to the agent (#142): the viewer side of the companion
//! worker (start, poll, `wake` with the delivery id) and the `:display`,
//! `:context` and `:quit-for-restart` mail of `to-viewer.edn`. The durable
//! queue stays authoritative: a command is popped only after it was applied,
//! and a wake failure never loses mail (contract §5).

use std::path::{Path, PathBuf};

use companion::worker::{self, Worker};
use edn::Value;
use scene::document::{self, MailBoundary, ViewState};

use crate::mailbox::Mailbox;

/// Everything the frame loop needs from the mail side. IO is the worker's
/// real file/process boundary; `wake` records the ids asked for.
pub struct Companion {
    root: PathBuf,
    inbox: Mailbox,
    pub worker: Option<Worker>,
    /// Delivery ids handed to the worker's `wake` command.
    pub woken: Vec<i64>,
    /// One diagnosis line for the status bar (the Lua `mail_status`).
    pub status: Option<String>,
}

fn worker_error(error: companion::Failure) -> String {
    error.message
}

impl Companion {
    /// A backend launch: capture the cursor before the companion exists, so a
    /// `:display` written during startup is consumed rather than skipped
    /// (the Lua `Mailbox.last_id` capture).
    pub fn start(root: &Path, restart: bool) -> Companion {
        let mut companion = Companion::mail_only(root);
        match worker::start(&root.to_string_lossy(), restart) {
            Ok(started) => {
                companion.status = Some("Companion: starting".into());
                companion.worker = Some(started);
            }
            Err(error) => {
                // The queue is authoritative; a viewer without a helper still
                // consumes mail, and the launch keeps the project's identity.
                companion.status = Some(format!("Companion: {}", worker_error(error)));
            }
        }
        companion
    }

    /// The mail queue without a helper process (a launch whose helper is
    /// absent, and the seam the mail semantics are tested at).
    pub fn mail_only(root: &Path) -> Companion {
        Companion {
            root: root.to_path_buf(),
            inbox: Mailbox::inbox(root),
            worker: None,
            woken: Vec::new(),
            status: None,
        }
    }

    /// Consume up to 32 worker events and report a phase change. Returns
    /// whether the worker reached `ready` on this poll (the identity record
    /// is written once, after ready).
    pub fn poll_worker(&mut self) -> bool {
        let Some(worker) = &mut self.worker else {
            return false;
        };
        let before = (worker.seen, worker.phase.clone());
        let host = worker::PollHost::real();
        if let Err(error) = worker::poll(worker, &host) {
            worker.phase = "failed".into();
            worker.message = Some(worker_error(error));
        }
        let changed =
            before.0 != worker.seen || worker.phase == "failed" || before.1 != worker.phase;
        if changed {
            self.status = Some(match &worker.message {
                Some(message) => format!("Companion: {message}"),
                None => format!("Companion: {}", worker.phase),
            });
        }
        changed && worker.phase == "ready" && before.1 != "ready"
    }

    /// `true` once the helper settled (stopped or failed) and exited.
    pub fn settled(&self) -> bool {
        self.worker
            .as_ref()
            .is_none_or(|w| matches!(w.phase.as_str(), "stopped" | "failed") && w.exited)
    }

    /// Ask the helper to stop; `keep` retains the companion session.
    pub fn stop(&mut self, keep: bool) -> Result<(), String> {
        match &mut self.worker {
            Some(worker) => worker::command(worker, "stop", keep).map_err(worker_error),
            None => Ok(()),
        }
    }

    /// `Worker.command(companion,'wake',{id=cmd.id})`: the delivery id is the
    /// token the wake contract acknowledges. Without a helper the request is
    /// only recorded; the durable queue already holds the mail.
    pub fn wake(&mut self, id: i64) -> Result<(), String> {
        self.woken.push(id);
        match &mut self.worker {
            Some(worker) => worker::wake(worker, id).map_err(worker_error),
            None => Ok(()),
        }
    }

    /// Take and apply every queued `to-viewer.edn` command, oldest first.
    /// `boundary` loads `:display` diagrams; `state` is the view a
    /// same-diagram display reapplies.
    pub fn consume(&mut self, state: &ViewState) -> (ViewState, bool) {
        let boundary = self.boundary();
        let mut current = state.clone();
        let mut changed = false;
        loop {
            let command = match self.inbox.take() {
                Ok(Some(command)) => command,
                Ok(None) => break,
                Err(error) => {
                    self.status = Some(format!("Mailbox: {error}"));
                    break;
                }
            };
            current = document::apply_mail(&current, &command, &boundary);
            changed = true;
        }
        (current, changed)
    }

    /// The `:display` boundary: resolve against the project root, canonical
    /// compare, and load a fresh state from the IR.
    pub fn boundary(&self) -> DisplayBoundary {
        DisplayBoundary {
            root: self.root.clone(),
        }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn inbox_path(&self) -> &Path {
        &self.inbox.path
    }
}

/// Loads a `:display` path exactly as a fresh document load would: the
/// metrics root is derived from the diagram (`metrics.root`), not from the
/// project, so a diagram elsewhere still joins its own snapshots.
pub struct DisplayBoundary {
    root: PathBuf,
}

impl MailBoundary for DisplayBoundary {
    fn resolve(&self, path: &str, root: Option<&str>) -> String {
        if path.starts_with('/') {
            return path.to_owned();
        }
        let base = root
            .map(str::to_owned)
            .unwrap_or_else(|| self.root.to_string_lossy().into_owned());
        format!("{base}/{path}")
    }

    fn canonical(&self, path: &str) -> String {
        std::fs::canonicalize(path)
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|_| path.to_owned())
    }

    fn load(&self, path: &str) -> Option<ViewState> {
        let text = std::fs::read_to_string(path).ok()?;
        let doc = scene::ir::read_document(&text).ok()?;
        let mtime = crate::watch::mtime(Path::new(path)).ok();
        let base = crate::metrics::root(Path::new(path));
        let (snapshots, stamp) = crate::metrics::snapshot(&base);
        let mut state = document::new_state(
            doc,
            mtime.map(Value::String),
            None,
            None,
            &snapshots,
            Some(stamp),
        );
        state.path = Some(path.to_owned());
        state.project_root = Some(self.root.to_string_lossy().into_owned());
        Some(state)
    }
}

/// The discussion-context mail for a proposal choice (`mail.send_context`).
pub fn context_mail(context: &Value) -> Vec<(Value, Value)> {
    match context {
        Value::Map(entries) => entries.clone(),
        _ => Vec::new(),
    }
}

/// Did the consumed mail ask for a restart? The durable queue's
/// `:quit-for-restart` is the only trigger; the app then stops the helper
/// with `keep`, saves the session and exits 0.
pub fn quit_requested(state: &ViewState) -> bool {
    state.quit_for_restart
}

#[cfg(test)]
mod tests {
    use super::*;
    use edn::record::get;

    #[test]
    fn a_display_path_resolves_against_the_project_root_unless_absolute() {
        let directory = std::env::temp_dir().join(format!(
            "uml mail {} {}",
            std::process::id(),
            companion::posix::monotonic_ms()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("second.edn"), "{:title \"Second\"}").unwrap();
        let companion = Companion {
            root: directory.clone(),
            inbox: Mailbox::inbox(&directory),
            worker: None,
            woken: Vec::new(),
            status: None,
        };
        let boundary = companion.boundary();
        let root = directory.to_string_lossy().into_owned();
        assert_eq!(
            format!("{root}/second.edn"),
            boundary.resolve("second.edn", None)
        );
        assert_eq!("/abs/x.edn", boundary.resolve("/abs/x.edn", None));
        // POSIX names: `\` is an ordinary character and `C:` no drive.
        assert_eq!(
            format!("{root}/odd\\name.edn"),
            boundary.resolve("odd\\name.edn", None)
        );
        assert_eq!(
            format!("{root}/C:/x.edn"),
            boundary.resolve("C:/x.edn", None)
        );
        // A state's own project root wins when it names one.
        assert_eq!(
            "/other/second.edn",
            boundary.resolve("second.edn", Some("/other"))
        );
        let loaded = boundary
            .load(&boundary.resolve("second.edn", None))
            .unwrap();
        assert_eq!(
            Some(&Value::String("Second".into())),
            get(loaded.document.as_ref().unwrap(), "title")
        );
        assert_eq!(
            Some(directory.to_string_lossy().into_owned()),
            loaded.project_root
        );
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn consuming_mail_pops_each_command_after_applying_it() {
        let directory = std::env::temp_dir().join(format!(
            "uml mail consume {} {}",
            std::process::id(),
            companion::posix::monotonic_ms()
        ));
        std::fs::create_dir_all(directory.join(".uml-viewer")).unwrap();
        std::fs::write(
            directory.join(".uml-viewer/to-viewer.edn"),
            "{:next-id 3 :queue [{:id 1 :op :context :context :real} {:id 2 :op :quit-for-restart}]}",
        )
        .unwrap();
        let mut companion = Companion::mail_only(&directory);
        let mut state = document::new_state(Value::Map(vec![]), None, None, None, &[], None);
        let (next, changed) = companion.consume(&state);
        assert!(changed);
        state = next;
        assert_eq!(
            Some(&Value::keyword("real")),
            state
                .discussion_context
                .as_ref()
                .and_then(|c| get(c, "context"))
        );
        assert!(super::quit_requested(&state));
        assert_eq!(2, state.mail_seen);
        let rest = companion.inbox.read();
        assert!(crate::mailbox::queue(&rest).is_empty());
        let (unchanged, changed) = companion.consume(&state);
        assert!(!changed);
        assert_eq!(2, unchanged.mail_seen);
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn a_wake_without_a_helper_is_recorded_and_keeps_the_mail() {
        let directory = std::env::temp_dir().join(format!(
            "uml mail wake {} {}",
            std::process::id(),
            companion::posix::monotonic_ms()
        ));
        std::fs::create_dir_all(directory.join(".uml-viewer")).unwrap();
        let mut companion = Companion::mail_only(&directory);
        companion.wake(4).unwrap();
        assert_eq!(vec![4], companion.woken);
        assert!(companion.settled());
        assert!(companion.stop(false).is_ok());
        let _ = std::fs::remove_dir_all(directory);
    }
}
