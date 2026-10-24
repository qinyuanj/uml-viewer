//! The companion worker (port of `love/uml/adapters/companion_worker.lua`).
//! One isolated process owns lifecycle side effects. The UI exchanges
//! generation-scoped files and never waits for subprocess stdout or cleanup.
//!
//! File protocol under `.uml-viewer/companion-worker-<token>.*`:
//! - `.request.edn`  `{:token :root :restart :base :viewer {:pid :start}}`
//! - `.command-<n>.edn` `{:token :operation :stop|:wake :extra ...}`
//! - `.event-<n>.edn` `{:token :phase :message}`, with phases `starting`,
//!   the progress stages, `ready`, `stopping`, `stopped` and `failed`.
//!
//! Every file is written to `.tmp` then renamed.

use crate::herdr::{Herdr, ProgressFn};
use crate::lifecycle;
use crate::mailbox::Mailbox;
use crate::posix::{self, Failure, Result};
use crate::record;
use edn::Value;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// A companion that has not reached `ready` within this bound is cancelled;
/// ownership is retained (Lua `M.poll`).
pub const STARTUP_TIMEOUT_MS: i64 = 120_000;
const POLL_MS: i64 = 50;
const MAX_EVENTS_PER_POLL: usize = 32;

/// The unique token of one mail delivery. Herdr's wake contract requires the
/// literal prompt to carry it.
pub fn mail_token(id: i64) -> String {
    format!("mail-{id}")
}

fn write(path: &Path, value: &Value) -> Result<()> {
    // The destination is unique per generation, never replaced.
    let temporary = PathBuf::from(format!("{}.tmp", path.display()));
    let encoded = edn::encode(value)?;
    std::fs::write(&temporary, encoded)?;
    std::fs::rename(&temporary, path)?;
    Ok(())
}

fn read(path: &Path) -> Option<Value> {
    let text = std::fs::read_to_string(path).ok()?;
    edn::parse(&text).ok()
}

/// A started worker, as the viewer tracks it.
pub struct Worker {
    pub base: String,
    pub token: String,
    pub owner: posix::Owner,
    pub sequence: u64,
    pub seen: u64,
    pub phase: String,
    pub message: Option<String>,
    pub started: Option<i64>,
    pub timed_out: bool,
    pub exited: bool,
}

/// The viewer-side start: write the request, then spawn the helper natively.
pub fn start(root: &str, restart: bool) -> Result<Worker> {
    let token = crate::recovery::new_token();
    let base = format!("{root}/.uml-viewer/companion-worker-{token}");
    let owner = posix::owner_native(posix::current_pid())?
        .ok_or_else(|| Failure::new("viewer process identity is unavailable"))?;
    let request = edn::record::record(vec![
        ("token", Value::String(token.clone())),
        ("root", Value::String(root.to_owned())),
        ("restart", Value::Bool(restart)),
        ("base", Value::String(base.clone())),
        (
            "viewer",
            edn::record::record(vec![
                ("pid", Value::Integer(i64::from(owner.pid))),
                ("start", Value::String(owner.start.clone())),
            ]),
        ),
    ]);
    let request_path = format!("{base}.request.edn");
    write(Path::new(&request_path), &request)?;
    let executable = std::env::current_exe()
        .map_err(|error| Failure::new(format!("cannot resolve the uml-viewer binary: {error}")))?;
    let argv = vec![
        std::ffi::OsString::from("companion-worker"),
        std::ffi::OsString::from(&request_path),
    ];
    let child = posix::spawn_native(executable.as_os_str(), &argv, Path::new(root))?;
    Ok(Worker {
        base,
        token,
        owner: child,
        sequence: 0,
        seen: 0,
        phase: "starting".to_owned(),
        message: None,
        started: Some(posix::monotonic_ms()),
        timed_out: false,
        exited: false,
    })
}

/// Ask the helper to stop (`keep` retains the companion session).
pub fn command(worker: &mut Worker, operation: &str, keep: bool) -> Result<()> {
    command_with(worker, operation, keep, &|path, value| {
        write(Path::new(path), value)
    })
}

/// Ask the helper to wake its pane for one mail delivery. The id is the
/// delivery token the wake contract acknowledges (contract §5); it must not
/// be the `:keep` shape of `stop`.
pub fn wake(worker: &mut Worker, id: i64) -> Result<()> {
    worker.sequence += 1;
    let path = format!("{}.command-{}.edn", worker.base, worker.sequence);
    write(
        Path::new(&path),
        &edn::record::record(vec![
            ("token", Value::String(worker.token.clone())),
            ("operation", Value::keyword("wake")),
            (
                "extra",
                edn::record::record(vec![("id", Value::Integer(id))]),
            ),
        ]),
    )
}

/// The same command, through an injected writer (the poll path uses the
/// host so tests and the viewer share one boundary).
fn command_with(
    worker: &mut Worker,
    operation: &str,
    keep: bool,
    write: &dyn Fn(&str, &Value) -> Result<()>,
) -> Result<()> {
    worker.sequence += 1;
    let path = format!("{}.command-{}.edn", worker.base, worker.sequence);
    write(
        &path,
        &edn::record::record(vec![
            ("token", Value::String(worker.token.clone())),
            ("operation", Value::keyword(operation)),
            (
                "extra",
                edn::record::record(vec![("keep", Value::Bool(keep))]),
            ),
        ]),
    )
}

/// The viewer-side boundary: event/command files and the helper's identity.
pub struct PollHost {
    pub read: Box<ReadFn>,
    pub write: Box<WriteFn>,
    pub owner: Box<OwnerFn>,
    pub clock: Box<ClockFn>,
}

type ReadFn = dyn Fn(&str) -> Option<Value>;
type WriteFn = dyn Fn(&str, &Value) -> Result<()>;
type OwnerFn = dyn Fn(i32) -> Option<posix::Owner>;
type ClockFn = dyn Fn() -> i64;

impl PollHost {
    pub fn real() -> PollHost {
        PollHost {
            read: Box::new(|path| read(Path::new(path))),
            write: Box::new(|path, value| write(Path::new(path), value)),
            owner: Box::new(|pid| posix::owner_native(pid).ok().flatten()),
            clock: Box::new(posix::monotonic_ms),
        }
    }
}

/// Consume up to 32 matching events, enforce the startup deadline once, and
/// notice a helper that exited without a final result. A stale generation
/// result is an error before the cursor advances.
pub fn poll(worker: &mut Worker, host: &PollHost) -> Result<()> {
    let start = worker.seen;
    while worker.seen - start < MAX_EVENTS_PER_POLL as u64 {
        let index = worker.seen + 1;
        let path = format!("{}.event-{index}.edn", worker.base);
        let Some(event) = (host.read)(&path) else {
            break;
        };
        if record::get_name(&event, "token").as_deref() != Some(worker.token.as_str()) {
            return Err(Failure::new(
                "Companion helper result identity mismatch; ownership retained",
            ));
        }
        worker.seen = index;
        let phase = record::get_name(&event, "phase").unwrap_or_default();
        if !worker.timed_out || phase == "stopped" || phase == "failed" {
            worker.phase = phase.clone();
            worker.message = record::get_name(&event, "message");
            if phase == "ready" {
                worker.started = None;
            }
        }
    }
    let pending = worker.started.is_some()
        && !worker.timed_out
        && !matches!(worker.phase.as_str(), "ready" | "failed" | "stopped");
    if pending && (host.clock)() - worker.started.unwrap_or(0) > STARTUP_TIMEOUT_MS {
        worker.timed_out = true;
        worker.phase = "failed".to_owned();
        worker.message = Some(
            "Companion startup timed out; cancellation pending, ownership retained".to_owned(),
        );
        command_with(worker, "stop", false, &*host.write)?;
    }
    let observed = (host.owner)(worker.owner.pid);
    let alive = observed.is_some_and(|observed| observed.start == worker.owner.start);
    if !alive {
        if !matches!(worker.phase.as_str(), "stopped" | "failed") {
            worker.phase = "failed".to_owned();
            worker.message = Some(
                "Companion helper exited without a final result; ownership retained".to_owned(),
            );
        }
        worker.exited = true;
    }
    Ok(())
}

/// The helper's generation state: one event journal, one command cursor and
/// the cancellation flags. The startup progress callback shares this state, so
/// a cancellation can land between create boundaries exactly as the Lua
/// worker's `progress` hook did.
struct Session {
    base: String,
    token: String,
    sequence: u64,
    next_command: u64,
    stop: bool,
    keep: bool,
    pending: Vec<i64>,
    viewer_pid: i32,
    viewer_start: String,
}

impl Session {
    fn event(&mut self, phase: &str, message: Option<String>) -> Result<()> {
        self.sequence += 1;
        let mut entries = vec![
            ("token", Value::String(self.token.clone())),
            ("phase", Value::keyword(phase)),
        ];
        if let Some(message) = message {
            entries.push(("message", Value::String(message)));
        }
        write(
            Path::new(&format!("{}.event-{}.edn", self.base, self.sequence)),
            &edn::record::record(entries),
        )
    }

    /// Drain every queued command, then notice a viewer that is gone.
    fn commands(&mut self) -> Result<()> {
        loop {
            let path = format!("{}.command-{}.edn", self.base, self.next_command);
            let Some(command) = read(Path::new(&path)) else {
                break;
            };
            if record::get_name(&command, "token").as_deref() != Some(self.token.as_str()) {
                return Err(Failure::new("Helper command identity mismatch"));
            }
            self.next_command += 1;
            let extra = edn::record::get(&command, "extra")
                .cloned()
                .unwrap_or(Value::Nil);
            match record::get_name(&command, "operation").as_deref() {
                Some("stop") => {
                    self.stop = true;
                    self.keep = edn::record::is_true(&extra, "keep");
                }
                Some("wake") => {
                    if let Some(id) = record::get_i64(&extra, "id") {
                        self.pending.push(id);
                    }
                }
                _ => {}
            }
        }
        let viewer = posix::owner_native(self.viewer_pid)?;
        if !viewer.is_some_and(|viewer| viewer.start == self.viewer_start) {
            self.stop = true;
        }
        Ok(())
    }

    /// One create boundary: publish the stage, then honour a cancellation that
    /// arrived since the previous boundary.
    fn progress(&mut self, stage: &str) -> Result<()> {
        self.event(stage, None)?;
        self.commands()?;
        if self.stop {
            return Err(Failure::new("Companion startup cancelled"));
        }
        Ok(())
    }
}

/// The helper role body. `open_window` is `None` when the viewer asked for no
/// terminal attach (`UML_COMPANION_NO_ATTACH=1`).
pub fn run(request_path: &str, attach: bool) -> Result<i32> {
    let request = read(Path::new(request_path))
        .ok_or_else(|| Failure::new("Missing companion helper request"))?;
    let root = record::get_name(&request, "root").unwrap_or_default();
    let base = record::get_name(&request, "base").unwrap_or_default();
    let token = record::get_name(&request, "token").unwrap_or_default();
    let restart = edn::record::flag(&request, "restart");
    let viewer = edn::record::get(&request, "viewer")
        .cloned()
        .unwrap_or(Value::Nil);
    let session = Rc::new(RefCell::new(Session {
        base,
        token,
        sequence: 0,
        next_command: 1,
        stop: false,
        keep: false,
        pending: Vec::new(),
        viewer_pid: record::get_i64(&viewer, "pid").unwrap_or(0) as i32,
        viewer_start: record::get_name(&viewer, "start").unwrap_or_default(),
    }));
    let lock = posix::try_acquire(Path::new(&format!(
        "{root}/.uml-viewer/companion-worker.lock"
    )))?;
    if lock.is_none() {
        session.borrow_mut().event(
            "failed",
            Some("Another companion helper owns this project; ownership retained".to_owned()),
        )?;
        return Ok(1);
    }
    let progress: ProgressFn = {
        let session = session.clone();
        Rc::new(move |stage: &str| session.borrow_mut().progress(stage))
    };
    let outcome = run_body(&session, &root, restart, attach, progress);
    let (stop, keep) = {
        let session = session.borrow();
        (session.stop, session.keep)
    };
    match outcome {
        Ok(()) => {}
        Err(error) => {
            if stop && !keep {
                // A completed record may be released only through the
                // verified cleanup contract; partial records remain.
                if lifecycle::required(&root).is_ok() {
                    match lifecycle::shutdown(&root) {
                        Ok(_) => {
                            session.borrow_mut().event(
                                "stopped",
                                Some("Startup cancelled; owned resources released".to_owned()),
                            )?;
                        }
                        Err(error) => {
                            session.borrow_mut().event(
                                "failed",
                                Some(format!("Startup cancelled: {error}; ownership retained")),
                            )?;
                        }
                    }
                } else {
                    session.borrow_mut().event(
                        "failed",
                        Some(format!("{error}; incomplete ownership retained")),
                    )?;
                }
            } else {
                session
                    .borrow_mut()
                    .event("failed", Some(format!("{error}; ownership retained")))?;
            }
            posix::release(lock);
            return Ok(1);
        }
    }
    posix::release(lock);
    Ok(0)
}

/// The helper body. Startup stages arrive through the backend's progress
/// callback, so each stage is published at the boundary it names and a
/// cancellation can land between boundaries.
fn run_body(
    session: &Rc<RefCell<Session>>,
    root: &str,
    restart: bool,
    attach: bool,
    progress: ProgressFn,
) -> Result<()> {
    session.borrow_mut().event("starting", None)?;
    let (record_value, backend): (Value, Herdr) = if restart {
        lifecycle::restart(root)?
    } else {
        lifecycle::create(
            root,
            lifecycle::CreateOptions {
                progress: Some(progress),
                ..lifecycle::CreateOptions::default()
            },
        )?
    };
    // A restart has no create boundaries, so drain once before publishing
    // ready; a cancellation or a vanished viewer still lands here.
    session.borrow_mut().commands()?;
    if session.borrow().stop {
        return Err(Failure::new("Companion startup cancelled"));
    }
    let ready = if !restart && attach {
        let name = record::get_name(&record_value, "session").unwrap_or_default();
        let opened = lifecycle::open_window(root, &name, &backend.herdr());
        if opened.terminal == "none" {
            Some(format!(
                "Companion started; attach manually: {}",
                opened.attach
            ))
        } else {
            None
        }
    } else {
        None
    };
    session.borrow_mut().event("ready", ready)?;
    let mailbox = Mailbox::outbox(Path::new(root));
    loop {
        session.borrow_mut().commands()?;
        let id = {
            let mut state = session.borrow_mut();
            if state.stop || state.pending.is_empty() {
                None
            } else {
                Some(state.pending.remove(0))
            }
        };
        let Some(id) = id else {
            if session.borrow().stop {
                break;
            }
            posix::sleep(POLL_MS);
            continue;
        };
        let token = mail_token(id);
        let pane = record::get_name(&record_value, "pane").unwrap_or_default();
        let delivered = backend.wake(
            &pane,
            &["-l".to_owned(), format!("Mailbox {token}")],
            &token,
            &|token| mailbox.acknowledged_id(token),
            Some(5000),
            None,
        );
        if let Err(error) = delivered {
            session
                .borrow_mut()
                .event("ready", Some(format!("Companion wake failed: {error}")))?;
        }
    }
    session.borrow_mut().event("stopping", None)?;
    let keep = session.borrow().keep;
    if !keep {
        lifecycle::shutdown(root)?;
    }
    session.borrow_mut().event("stopped", None)?;
    Ok(())
}

impl Mailbox {
    /// Consumption proof by the literal token's mailbox id.
    fn acknowledged_id(&self, token: &str) -> bool {
        token
            .strip_prefix("mail-")
            .and_then(|id| id.parse::<i64>().ok())
            .is_some_and(|id| self.acknowledged(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;

    #[test]
    fn names_a_mail_delivery_by_its_mailbox_id() {
        assert_eq!("mail-7", mail_token(7));
        assert_eq!("mail-12", mail_token(12));
    }

    struct Journal {
        files: Rc<RefCell<HashMap<String, Value>>>,
        events: Rc<RefCell<Vec<Value>>>,
    }

    fn host(
        journal: &Journal,
        owner: impl Fn(i32) -> Option<posix::Owner> + 'static,
        now: Rc<RefCell<i64>>,
    ) -> PollHost {
        let files = journal.files.clone();
        let written = journal.events.clone();
        let write_files = journal.files.clone();
        PollHost {
            read: Box::new(move |path| files.borrow().get(path).cloned()),
            write: Box::new(move |path, value| {
                written.borrow_mut().push(value.clone());
                // The helper journal is in-memory: a written command/event is
                // visible to the next read, exactly like the file protocol.
                write_files
                    .borrow_mut()
                    .insert(path.trim_end_matches(".tmp").to_owned(), value.clone());
                Ok(())
            }),
            owner: Box::new(owner),
            clock: Box::new(move || *now.borrow()),
        }
    }

    fn worker() -> Worker {
        Worker {
            base: "journal".to_owned(),
            token: "generation".to_owned(),
            owner: posix::Owner {
                pid: 2,
                start: "original".to_owned(),
                name: String::new(),
            },
            sequence: 0,
            seen: 0,
            phase: "starting".to_owned(),
            message: None,
            started: None,
            timed_out: false,
            exited: false,
        }
    }

    fn poll_with(events: Vec<Value>, observed: Option<posix::Owner>) -> Worker {
        let journal = Journal {
            files: Rc::new(RefCell::new(HashMap::new())),
            events: Rc::new(RefCell::new(Vec::new())),
        };
        for (index, event) in events.into_iter().enumerate() {
            journal
                .files
                .borrow_mut()
                .insert(format!("journal.event-{}.edn", index + 1), event);
        }
        let now = Rc::new(RefCell::new(0i64));
        let host = host(&journal, move |_| observed.clone(), now);
        let mut worker = worker();
        poll(&mut worker, &host).unwrap();
        worker
    }

    fn event(token: &str, phase: &str) -> Value {
        edn::record::record(vec![
            ("token", Value::String(token.to_owned())),
            ("phase", Value::keyword(phase)),
        ])
    }

    #[test]
    fn consumes_matching_startup_events_in_order() {
        let worker = poll_with(
            vec![event("generation", "server"), event("generation", "ready")],
            Some(posix::Owner {
                pid: 2,
                start: "original".to_owned(),
                name: String::new(),
            }),
        );
        assert_eq!(2, worker.seen);
        assert_eq!("ready", worker.phase);
        assert!(!worker.exited);
    }

    #[test]
    fn rejects_stale_generation_results_before_advancing_the_cursor() {
        let journal = Journal {
            files: Rc::new(RefCell::new(HashMap::new())),
            events: Rc::new(RefCell::new(Vec::new())),
        };
        journal
            .files
            .borrow_mut()
            .insert("journal.event-1.edn".to_owned(), event("stale", "ready"));
        let now = Rc::new(RefCell::new(0i64));
        let host = host(
            &journal,
            |_| {
                Some(posix::Owner {
                    pid: 2,
                    start: "original".to_owned(),
                    name: String::new(),
                })
            },
            now,
        );
        let mut worker = worker();
        let error = poll(&mut worker, &host).unwrap_err();
        assert_eq!(
            "Companion helper result identity mismatch; ownership retained",
            error.message
        );
        assert_eq!(0, worker.seen);
        assert_eq!("starting", worker.phase);
    }

    #[test]
    fn reports_an_unknown_helper_exit_without_inventing_success() {
        let worker = poll_with(vec![], None);
        assert_eq!("failed", worker.phase);
        assert!(worker.exited);
        assert!(
            worker
                .message
                .as_deref()
                .unwrap_or_default()
                .contains("without a final result")
        );
    }

    #[test]
    fn rejects_a_reused_helper_pid() {
        let worker = poll_with(
            vec![event("generation", "ready")],
            Some(posix::Owner {
                pid: 2,
                start: "replacement".to_owned(),
                name: String::new(),
            }),
        );
        assert_eq!("failed", worker.phase);
        assert!(worker.exited);
    }

    #[test]
    fn preserves_a_published_failure_after_helper_exit() {
        let worker = poll_with(
            vec![edn::record::record(vec![
                ("token", Value::String("generation".to_owned())),
                ("phase", Value::keyword("failed")),
                (
                    "message",
                    Value::String("workspace failed; ownership retained".to_owned()),
                ),
            ])],
            None,
        );
        assert_eq!("failed", worker.phase);
        assert!(worker.exited);
        assert_eq!(
            Some("workspace failed; ownership retained".to_owned()),
            worker.message
        );
    }

    #[test]
    fn recognizes_a_confirmed_stop_after_helper_exit() {
        let worker = poll_with(vec![event("generation", "stopped")], None);
        assert_eq!("stopped", worker.phase);
        assert!(worker.exited);
    }

    #[test]
    fn requests_cancellation_once_when_startup_exceeds_its_deadline() {
        let journal = Journal {
            files: Rc::new(RefCell::new(HashMap::new())),
            events: Rc::new(RefCell::new(Vec::new())),
        };
        let now = Rc::new(RefCell::new(120101i64));
        let host = host(
            &journal,
            |_| {
                Some(posix::Owner {
                    pid: 2,
                    start: "original".to_owned(),
                    name: String::new(),
                })
            },
            now,
        );
        let mut worker = worker();
        worker.started = Some(100);
        poll(&mut worker, &host).unwrap();
        poll(&mut worker, &host).unwrap();
        assert!(worker.timed_out);
        assert_eq!("failed", worker.phase);
        assert!(
            worker
                .message
                .as_deref()
                .unwrap_or_default()
                .contains("cancellation pending")
        );
        let cancels = journal
            .events
            .borrow()
            .iter()
            .filter(|event| record::get_name(event, "operation").as_deref() == Some("stop"))
            .count();
        assert_eq!(1, cancels);
        let extra = edn::record::get(journal.events.borrow().last().unwrap(), "extra")
            .cloned()
            .unwrap();
        assert_eq!(Some(&Value::Bool(false)), edn::record::get(&extra, "keep"));
    }

    #[test]
    fn ignores_late_ready_after_timeout_but_accepts_confirmed_cleanup() {
        let journal = Journal {
            files: Rc::new(RefCell::new(HashMap::new())),
            events: Rc::new(RefCell::new(Vec::new())),
        };
        journal.files.borrow_mut().insert(
            "journal.event-1.edn".to_owned(),
            event("generation", "ready"),
        );
        let now = Rc::new(RefCell::new(0i64));
        let host = host(
            &journal,
            |_| {
                Some(posix::Owner {
                    pid: 2,
                    start: "original".to_owned(),
                    name: String::new(),
                })
            },
            now,
        );
        let mut worker = worker();
        worker.timed_out = true;
        worker.phase = "failed".to_owned();
        worker.message = Some("cancellation pending".to_owned());
        poll(&mut worker, &host).unwrap();
        assert_eq!("failed", worker.phase);
        assert_eq!(Some("cancellation pending".to_owned()), worker.message);
        journal.files.borrow_mut().insert(
            "journal.event-2.edn".to_owned(),
            edn::record::record(vec![
                ("token", Value::String("generation".to_owned())),
                ("phase", Value::keyword("stopped")),
                (
                    "message",
                    Value::String("owned resources released".to_owned()),
                ),
            ]),
        );
        poll(&mut worker, &host).unwrap();
        assert_eq!("stopped", worker.phase);
        assert_eq!(Some("owned resources released".to_owned()), worker.message);
    }

    #[test]
    fn disables_the_startup_deadline_after_ready() {
        let journal = Journal {
            files: Rc::new(RefCell::new(HashMap::new())),
            events: Rc::new(RefCell::new(Vec::new())),
        };
        journal.files.borrow_mut().insert(
            "journal.event-1.edn".to_owned(),
            event("generation", "ready"),
        );
        let now = Rc::new(RefCell::new(120101i64));
        let host = host(
            &journal,
            |_| {
                Some(posix::Owner {
                    pid: 2,
                    start: "original".to_owned(),
                    name: String::new(),
                })
            },
            now,
        );
        let mut worker = worker();
        worker.started = Some(100);
        poll(&mut worker, &host).unwrap();
        assert_eq!(None, worker.started);
        assert!(!worker.timed_out);
        journal.files.borrow_mut().insert(
            "journal.event-2.edn".to_owned(),
            event("generation", "stopping"),
        );
        poll(&mut worker, &host).unwrap();
        assert_eq!("stopping", worker.phase);
        assert!(!worker.timed_out);
    }

    #[test]
    fn the_worker_file_protocol_writes_then_renames_every_event() {
        let directory = std::env::temp_dir().join(format!(
            "uml worker {} {}",
            std::process::id(),
            crate::posix::monotonic_ms()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let path = directory.join("event-1.edn");
        write(&path, &event("generation", "ready")).unwrap();
        assert!(!directory.join("event-1.edn.tmp").exists());
        assert_eq!(
            Some("ready".to_owned()),
            record::get_name(&read(&path).unwrap(), "phase")
        );
        assert!(read(&directory.join("missing.edn")).is_none());
        let _ = std::fs::remove_dir_all(&directory);
    }

    fn session_in(directory: &Path) -> Rc<RefCell<Session>> {
        let base = format!("{}/journal", directory.display());
        Rc::new(RefCell::new(Session {
            base,
            token: "generation".to_owned(),
            sequence: 0,
            next_command: 1,
            stop: false,
            keep: false,
            pending: Vec::new(),
            viewer_pid: posix::current_pid(),
            viewer_start: posix::owner_native(posix::current_pid())
                .unwrap()
                .expect("viewer identity")
                .start,
        }))
    }

    fn phases(directory: &Path) -> Vec<String> {
        let mut found: Vec<(u64, String)> = std::fs::read_dir(directory)
            .unwrap()
            .filter_map(|entry| {
                let name = entry.ok()?.file_name().to_string_lossy().into_owned();
                let index: u64 = name
                    .strip_prefix("journal.event-")?
                    .strip_suffix(".edn")?
                    .parse()
                    .ok()?;
                let value = read(&directory.join(&name))?;
                Some((index, record::get_name(&value, "phase")?))
            })
            .collect();
        found.sort();
        found.into_iter().map(|(_, phase)| phase).collect()
    }

    #[test]
    fn a_startup_stage_is_published_at_its_boundary_and_a_queued_stop_cancels() {
        let directory = std::env::temp_dir().join(format!(
            "uml worker progress {} {}",
            std::process::id(),
            crate::posix::monotonic_ms()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let session = session_in(&directory);
        session.clone().borrow_mut().progress("server").unwrap();
        // The command arrives while the workspace boundary is still being
        // created; it must land before the next stage runs.
        write(
            &directory.join("journal.command-1.edn"),
            &edn::record::record(vec![
                ("token", Value::String("generation".to_owned())),
                ("operation", Value::keyword("stop")),
                (
                    "extra",
                    edn::record::record(vec![("keep", Value::Bool(true))]),
                ),
            ]),
        )
        .unwrap();
        let error = session
            .clone()
            .borrow_mut()
            .progress("workspace")
            .unwrap_err();
        assert_eq!("Companion startup cancelled", error.message);
        assert!(session.borrow().stop);
        assert!(session.borrow().keep);
        assert_eq!(vec!["server", "workspace"], phases(&directory));
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_wake_command_does_not_interrupt_the_startup_stages() {
        let directory = std::env::temp_dir().join(format!(
            "uml worker wake {} {}",
            std::process::id(),
            crate::posix::monotonic_ms()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let session = session_in(&directory);
        write(
            &directory.join("journal.command-1.edn"),
            &edn::record::record(vec![
                ("token", Value::String("generation".to_owned())),
                ("operation", Value::keyword("wake")),
                (
                    "extra",
                    edn::record::record(vec![("id", Value::Integer(4))]),
                ),
            ]),
        )
        .unwrap();
        session.clone().borrow_mut().progress("runner").unwrap();
        assert!(!session.borrow().stop);
        assert_eq!(vec![4i64], session.borrow().pending);
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn a_second_helper_for_the_same_project_is_refused() {
        let directory = std::env::temp_dir().join(format!(
            "uml worker lock {} {}",
            std::process::id(),
            crate::posix::monotonic_ms()
        ));
        std::fs::create_dir_all(directory.join(".uml-viewer")).unwrap();
        let root = directory.to_string_lossy().into_owned();
        let request_path = format!("{root}/.uml-viewer/companion-worker-gen.request.edn");
        write(
            Path::new(&request_path),
            &edn::record::record(vec![
                ("token", Value::String("gen".to_owned())),
                ("root", Value::String(root.clone())),
                (
                    "base",
                    Value::String(format!("{root}/.uml-viewer/companion-worker-gen")),
                ),
                ("restart", Value::Bool(false)),
                (
                    "viewer",
                    edn::record::record(vec![
                        ("pid", Value::Integer(i64::from(posix::current_pid()))),
                        ("start", Value::String("start".to_owned())),
                    ]),
                ),
            ]),
        )
        .unwrap();
        let held = posix::try_acquire(Path::new(&format!(
            "{root}/.uml-viewer/companion-worker.lock"
        )))
        .unwrap()
        .expect("lock");
        let code = run(&request_path, false).unwrap();
        assert_eq!(1, code);
        let event_path = format!("{root}/.uml-viewer/companion-worker-gen.event-1.edn");
        let event = read(Path::new(&event_path)).unwrap();
        assert_eq!(Some("failed".to_owned()), record::get_name(&event, "phase"));
        assert!(
            record::get_name(&event, "message")
                .unwrap_or_default()
                .contains("Another companion helper")
        );
        posix::release(Some(held));
        let _ = std::fs::remove_dir_all(&directory);
    }
}
