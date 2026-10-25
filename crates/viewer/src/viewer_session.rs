//! The viewer session (#161): the window-free part of the viewer. It holds
//! the view state and consumes `to-viewer.edn` mail, and answers each
//! event with the repaint and GUI-cache requests the eframe shell carries
//! out. It reads and writes the real project files, so it is not pure; its
//! only host seam is the [`CompanionLink`], and the clock is an input.
//!
//! Not the session snapshot (`session`, what a restart restores) and not
//! the Herdr session.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use companion::mailbox::Mailbox;
use edn::Value;
use scene::document::{self, ViewState};
use scene::events::{self, Dims};

use crate::clock::{self, Now};
use crate::mail::DisplayBoundary;
use crate::session::{self, Chrome};
use crate::{identity, inspector, mail_watch, mailbox, metrics, project_files, strings, watch};

/// What the session needs from the companion helper: poll its events, stop
/// it (keeping the companion or not), wake it for a delivery id, and tell
/// whether it settled. The production adapter wraps the real worker (a
/// mail-only launch is one with no helper); tests use a fake.
pub(crate) trait CompanionLink {
    /// The helper's diagnosis line at launch (`Companion: starting`, or why
    /// it did not start).
    fn launch_status(&self) -> Option<String>;
    /// Consume pending worker events.
    fn poll(&mut self) -> LinkPoll;
    /// Ask the helper to stop; `keep` retains the companion.
    fn stop(&mut self, keep: bool) -> Result<(), String>;
    /// Wake the helper for the delivery `id` (the durable queue holds it).
    fn wake(&mut self, id: i64) -> Result<(), String>;
    /// `true` once the helper stopped or failed and exited (or never ran).
    fn settled(&self) -> bool;
}

/// One worker poll: did the helper reach ready on it, and its new status
/// line when its phase or message changed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct LinkPoll {
    pub ready: bool,
    pub status: Option<String>,
}

/// The launch settings the session opens with.
pub(crate) struct Launch {
    pub project_root: PathBuf,
    pub diagram: PathBuf,
    /// `--restart`: restore the saved session snapshot.
    pub restart: bool,
    /// `--backend herdr`: show the waiting canvas until `:display`.
    pub backend: bool,
}

/// Something the shell hands the session.
pub(crate) enum Event {
    /// A front-end action, with the canvas geometry at the time it was made.
    Action(Action, Dims),
    /// The mail watcher saw a change (or the startup frames).
    MailDue,
    /// A frame: hot reload when the poll cadence elapsed or a refresh is due.
    Tick,
    /// `R`: reload on the next tick regardless of the cadence.
    ForceRefresh,
    /// The frame's restart-exit checkpoint: after `:quit-for-restart`, stop
    /// the helper keeping the companion, poll it until it settled, then save
    /// the session snapshot (with this window chrome) and close with 0.
    RestartExit(Chrome),
    /// A normal close (the close control, a termination signal, the test
    /// duration): stop the helper without keeping the companion.
    Quit,
    /// The frame's normal-close checkpoint: close once the helper settled,
    /// or at the deadline regardless.
    Quitting,
    /// The window closed without the frame loop seeing it (a WM quit).
    WindowClosed,
}

/// How long a normal close waits for the helper to settle.
pub const QUIT_DEADLINE: Duration = Duration::from_secs(5);

/// What the inspector, class card and menus ask the session to do. Opening
/// a source window is the shell's own (it is GUI state).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Action {
    /// Choose the real diagram (`None`) or a proposal as the discussion.
    Choose(Option<Value>),
    NewProposal,
    Declutter,
    Regen,
    Rename(Value, String),
    Delete(Value),
    CloseCard,
    Locate(Value),
    /// A right-click menu operation on the selection: mail it to the agent.
    Mail(String),
}

/// The hot-reload and mail cadence.
pub const POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Whether the shell should draw another frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Repaint {
    #[default]
    None,
    Now,
    /// Draw again after this long (a stopping helper is polled).
    After(Duration),
}

/// The side effects an event asks the shell for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Effects {
    pub repaint: Repaint,
    /// The inspector and card caches are stale (new document or proposals).
    pub invalidate_gui: bool,
    /// Close the window with this exit code.
    pub close: Option<u8>,
}

pub(crate) struct ViewerSession {
    state: ViewState,
    project_root: PathBuf,
    diagram: PathBuf,
    hint: Option<String>,
    status: Option<String>,
    link: Box<dyn CompanionLink>,
    /// The latest helper or mailbox line; copied to `status` on a mail poll.
    link_status: Option<String>,
    inbox: Mailbox,
    mail_watch: mail_watch::Watch,
    /// The identity was recorded once the helper reached ready.
    identity_recorded: bool,
    /// When the IR and metrics stamps were last polled; `None` is due.
    last_poll: Option<Instant>,
    force_refresh: bool,
    /// `:quit-for-restart` already asked the helper to stop with `keep`.
    restart_stop: bool,
    /// A normal close stops the helper; the viewer exits at this deadline.
    quit_deadline: Option<Instant>,
}

impl ViewerSession {
    /// Load the first view, then start the companion link, then adopt the
    /// mail files as seen. The view (and its mail cursor) exists before the
    /// helper does, so a `:display` written during startup is consumed.
    pub(crate) fn open(
        launch: Launch,
        start_link: impl FnOnce() -> Box<dyn CompanionLink>,
    ) -> Result<ViewerSession, String> {
        let mut session = ViewerSession {
            state: ViewState::default(),
            inbox: Mailbox::inbox(&launch.project_root),
            project_root: launch.project_root,
            diagram: launch.diagram,
            hint: None,
            status: None,
            link: Box::new(NoLink),
            link_status: None,
            mail_watch: mail_watch::Watch::new(),
            identity_recorded: false,
            last_poll: None,
            force_refresh: false,
            restart_stop: false,
            quit_deadline: None,
        };
        session.load_initial(launch.restart, launch.backend)?;
        session.link = start_link();
        session.link_status = session.link.launch_status();
        session.mail_watch.baseline(&session.project_root);
        Ok(session)
    }

    /// Apply one event at `now`.
    pub(crate) fn process(&mut self, event: Event, now: Now) -> Effects {
        match event {
            Event::Action(action, dims) => self.apply(action, &dims, now),
            Event::MailDue => self.poll_mail(),
            Event::Tick => {
                let due = self
                    .last_poll
                    .is_none_or(|at| now.at.saturating_duration_since(at) >= POLL_INTERVAL);
                if self.force_refresh || due {
                    self.force_refresh = false;
                    self.last_poll = Some(now.at);
                    self.reload();
                }
                // The frame that ran this tick draws the result; a reload
                // never asks for another one.
                Effects::default()
            }
            Event::ForceRefresh => {
                self.force_refresh = true;
                Effects::default()
            }
            Event::RestartExit(chrome) => self.restart_exit(&chrome),
            Event::Quit => self.begin_quit(now),
            Event::Quitting => self.quitting(now),
            Event::WindowClosed => {
                // The helper then releases the session and ownership itself.
                if !self.link.settled()
                    && let Err(error) = self.link.stop(false)
                {
                    eprintln!("uml viewer: could not stop the companion: {error}");
                }
                Effects::default()
            }
        }
    }

    /// `:quit-for-restart`: stop the helper with `keep` (a failed helper needs
    /// no stop and must not strand the restart), poll until it settled so the
    /// replacement viewer binds the companion that is actually there, then
    /// save the snapshot and exit 0.
    fn restart_exit(&mut self, chrome: &Chrome) -> Effects {
        if !self.state.quit_for_restart && !self.restart_stop {
            return Effects::default();
        }
        if !self.restart_stop {
            if let Err(error) = self.link.stop(true) {
                eprintln!("uml viewer: could not stop the companion: {error}");
            }
            self.restart_stop = true;
        }
        self.poll_link();
        if !self.link.settled() {
            return Effects {
                repaint: Repaint::After(POLL_INTERVAL),
                ..Effects::default()
            };
        }
        let snapshot = session::snapshot(&self.state, chrome);
        if let Err(error) = session::save(&self.project_root, &snapshot) {
            eprintln!("uml viewer: could not save the session: {error}");
        }
        Effects {
            close: Some(0),
            ..Effects::default()
        }
    }

    /// A normal close asks the helper to stop without keeping the companion;
    /// the helper then cancels recovery, removes its Herdr session and clears
    /// the ownership record. A viewer without a helper has nothing to stop.
    fn begin_quit(&mut self, now: Now) -> Effects {
        if self.quit_deadline.is_some() {
            return Effects::default();
        }
        self.quit_deadline = Some(now.at + QUIT_DEADLINE);
        if let Err(error) = self.link.stop(false) {
            eprintln!("uml viewer: could not stop the companion: {error}");
        }
        Effects {
            close: self.link.settled().then_some(0),
            ..Effects::default()
        }
    }

    /// Wait (bounded) for the helper to settle, then exit 0.
    fn quitting(&mut self, now: Now) -> Effects {
        let Some(deadline) = self.quit_deadline else {
            return Effects::default();
        };
        let settled = self.poll_link() || self.link.settled();
        if settled || now.at >= deadline {
            Effects {
                close: Some(0),
                ..Effects::default()
            }
        } else {
            // Poll the helper's events while it stops.
            Effects {
                repaint: Repaint::After(POLL_INTERVAL),
                ..Effects::default()
            }
        }
    }

    /// Apply one front-end action. Every action redraws; proposal writeback
    /// also invalidates the GUI caches.
    fn apply(&mut self, action: Action, dims: &Dims, now: Now) -> Effects {
        let outbox = Mailbox::outbox(&self.project_root);
        let mut effects = Effects {
            repaint: Repaint::Now,
            ..Effects::default()
        };
        let state = &self.state;
        match action {
            Action::Choose(id) => {
                let choice = inspector::choose(state, id.as_ref());
                self.state = choice.state;
                let extra: Vec<(Value, Value)> = match &choice.context {
                    Value::Map(entries) => entries.clone(),
                    _ => Vec::new(),
                };
                if let Err(error) = outbox.append("context", &extra) {
                    eprintln!("uml viewer: could not send the context: {error}");
                }
                self.state.discussion_context = Some(choice.context);
            }
            Action::NewProposal => {
                let name = clock::proposal_name(now.epoch);
                self.state =
                    events::add_proposal(state, now.epoch, &name, Some(&project_files::Files));
                effects.invalidate_gui = true;
            }
            Action::Declutter => self.state = events::cycle_declutter(state),
            Action::Regen => {
                // The durable queue is authoritative; the wake tells the
                // agent there is something to consume. Without a helper the
                // request still waits in the queue (#142).
                self.status = Some(match outbox.append("regen", &[]) {
                    Ok(cmd) => {
                        let id = companion::record::get_i64(&cmd, "id").unwrap_or(0);
                        if self.link.wake(id).is_ok() {
                            "Regen requested.".into()
                        } else {
                            "Regen queued; Claude session not attached.".into()
                        }
                    }
                    Err(_) => "Regen failed.".into(),
                });
                self.mail_watch.baseline(&self.project_root);
            }
            Action::Rename(id, name) => {
                self.state =
                    events::rename_proposal(state, Some(&id), &name, Some(&project_files::Files));
                effects.invalidate_gui = true;
            }
            Action::Delete(id) => {
                self.state = events::delete_proposal(state, Some(&id), Some(&project_files::Files));
                effects.invalidate_gui = true;
            }
            Action::CloseCard => self.state = events::close_detail(state),
            Action::Locate(id) => self.state = events::locate(state, &id, dims),
            Action::Mail(op) => {
                // The right-click menu writes the exact `to-agent.edn`
                // command and wakes the companion for that delivery.
                let (op, extra) = mailbox::menu_mail(state, state.selected.as_ref(), &op);
                match outbox.append(&op, &extra) {
                    Ok(cmd) => {
                        let id = companion::record::get_i64(&cmd, "id").unwrap_or(0);
                        if let Err(error) = self.link.wake(id) {
                            eprintln!("uml viewer: could not wake the companion: {error}");
                        }
                    }
                    Err(error) => self.status = Some(format!("Mail failed: {error}")),
                }
                self.mail_watch.baseline(&self.project_root);
            }
        }
        effects
    }

    /// Poll the IR and metrics stamps; reload when either changed.
    fn reload(&mut self) {
        // A companion launch shows the empty canvas until :display (#142).
        if self.state.waiting {
            return;
        }
        let path_text = self.diagram.to_string_lossy().into_owned();
        let Ok(mtime) = watch::mtime(&self.diagram) else {
            self.hint = Some(strings::missing_file(&path_text));
            return;
        };
        let (snapshots, stamp) = metrics::snapshot(&self.project_root);
        let current_mtime = Some(Value::String(mtime));
        if current_mtime == self.state.mtime && Some(stamp.clone()) == self.state.metrics_stamp {
            return;
        }
        let Ok(text) = std::fs::read_to_string(&self.diagram) else {
            self.hint = Some(strings::missing_file(&path_text));
            return;
        };
        let (next, _, rejected) =
            document::reload(&self.state, &text, current_mtime, &snapshots, Some(stamp));
        if let Some(reason) = rejected {
            eprintln!("Document reload rejected: {reason}");
        }
        if next.document.is_some() {
            self.hint = None;
        }
        self.state = next;
    }

    pub(crate) fn state(&self) -> &ViewState {
        &self.state
    }

    pub(crate) fn state_mut(&mut self) -> &mut ViewState {
        &mut self.state
    }

    /// The canvas hint (a missing diagram), if any.
    pub(crate) fn hint(&self) -> Option<&str> {
        self.hint.as_deref()
    }

    /// The status line, if any.
    pub(crate) fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    pub(crate) fn set_status(&mut self, status: String) {
        self.status = Some(status);
    }

    /// Poll the helper; `true` when it reached ready on this poll.
    fn poll_link(&mut self) -> bool {
        let polled = self.link.poll();
        if let Some(line) = polled.status {
            self.link_status = Some(line);
        }
        polled.ready
    }

    fn load_initial(&mut self, restart: bool, backend: bool) -> Result<(), String> {
        let (snapshots, stamp) = metrics::snapshot(&self.project_root);
        let path = self.diagram.to_string_lossy().into_owned();
        let root = self.project_root.to_string_lossy().into_owned();
        if backend && !restart {
            let mut state = document::waiting_state(Some(path), 0);
            state.project_root = Some(root);
            state.revision = 1;
            self.state = state;
            return Ok(());
        }
        let text = match std::fs::read_to_string(&self.diagram) {
            Ok(text) => text,
            Err(_) => {
                self.state = ViewState {
                    path: Some(path.clone()),
                    project_root: Some(root),
                    revision: 1,
                    ..ViewState::default()
                };
                self.hint = Some(strings::missing_file(&path));
                return Ok(());
            }
        };
        let doc = scene::ir::read_document(&text).map_err(|e| format!("{path}: {e}"))?;
        let mtime = watch::mtime(&self.diagram).ok();
        let mut state = document::new_state(
            doc,
            mtime.map(Value::String),
            None,
            None,
            &snapshots,
            Some(stamp),
        );
        state.path = Some(path);
        state.project_root = Some(root);
        if restart {
            if let Some(snapshot) = crate::session::read(&self.project_root) {
                state = crate::session::restore(&state, &snapshot);
            }
        } else {
            state.fit_pending = true;
        }
        self.state = state;
        Ok(())
    }

    /// The mail loop: consume worker events, then the `to-viewer.edn`
    /// queue. A poll with nothing new does no work and asks for no repaint,
    /// so an idle viewer stays idle (#142).
    fn poll_mail(&mut self) -> Effects {
        let mut effects = Effects::default();
        let found = self.mail_watch.poll(&self.project_root);
        let mut changed = false;
        if found.worker {
            let ready = self.poll_link();
            changed = true;
            if ready && !self.identity_recorded {
                // Identity is written once, after the companion is ready.
                if let Err(error) = identity::record(&self.project_root) {
                    eprintln!("uml viewer: could not record the process identity: {error}");
                }
                self.identity_recorded = true;
            }
        }
        if found.mail {
            if self.consume() {
                effects.invalidate_gui = true;
                changed = true;
            }
            // The pop rewrote the file this watcher reads.
            self.mail_watch.baseline(&self.project_root);
        }
        if let Some(status) = self.link_status.clone()
            && self.status.as_deref() != Some(status.as_str())
        {
            self.status = Some(status);
            changed = true;
        }
        if changed {
            effects.repaint = Repaint::Now;
        }
        effects
    }

    /// Take and apply every queued command, oldest first; each is popped
    /// only after it was applied, so the durable queue stays authoritative.
    fn consume(&mut self) -> bool {
        let boundary = DisplayBoundary::new(&self.project_root);
        let mut consumed = false;
        loop {
            let command = match self.inbox.take() {
                Ok(Some(command)) => command,
                Ok(None) => break,
                Err(error) => {
                    self.link_status = Some(format!("Mailbox: {error}"));
                    break;
                }
            };
            let next = document::apply_mail(&self.state, &command, &boundary);
            let path_changed = next.path != self.state.path;
            self.state = next;
            if let Some(path) = self.state.path.clone() {
                self.diagram = PathBuf::from(path);
            }
            if path_changed {
                self.hint = None;
            }
            consumed = true;
        }
        consumed
    }
}

/// The link before `open` starts the real one.
struct NoLink;

impl CompanionLink for NoLink {
    fn launch_status(&self) -> Option<String> {
        None
    }
    fn poll(&mut self) -> LinkPoll {
        LinkPoll::default()
    }
    fn stop(&mut self, _keep: bool) -> Result<(), String> {
        Ok(())
    }
    fn wake(&mut self, _id: i64) -> Result<(), String> {
        Ok(())
    }
    fn settled(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use edn::record::get;
    use std::cell::RefCell;
    use std::path::Path;
    use std::rc::Rc;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Instant;

    /// What the fake link was asked to do.
    #[derive(Default)]
    struct Calls {
        woken: Vec<i64>,
        stops: Vec<bool>,
        wake_fails: bool,
        /// The helper has not settled yet (it is still stopping).
        unsettled: bool,
        /// The next poll reports the helper reached ready.
        ready_next: bool,
        polls: usize,
    }

    /// The test adapter: no helper process; it records calls and fails a
    /// wake when told to.
    struct FakeLink(Rc<RefCell<Calls>>);

    impl CompanionLink for FakeLink {
        fn launch_status(&self) -> Option<String> {
            None
        }
        fn poll(&mut self) -> LinkPoll {
            let mut calls = self.0.borrow_mut();
            calls.polls += 1;
            LinkPoll {
                ready: std::mem::take(&mut calls.ready_next),
                status: None,
            }
        }
        fn stop(&mut self, keep: bool) -> Result<(), String> {
            self.0.borrow_mut().stops.push(keep);
            Ok(())
        }
        fn wake(&mut self, id: i64) -> Result<(), String> {
            let mut calls = self.0.borrow_mut();
            calls.woken.push(id);
            if calls.wake_fails {
                Err("no helper".into())
            } else {
                Ok(())
            }
        }
        fn settled(&self) -> bool {
            !self.0.borrow().unsettled
        }
    }

    /// A fresh project directory per call.
    fn project(label: &str) -> PathBuf {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "uml viewer session {label} {} {n}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join(".uml-viewer")).unwrap();
        dir
    }

    const SECOND: &str = "{:title \"Second diagram\" :packages [{:label \"P\" :classes [{:id :target :ns \"app.target\"}]}]}";

    fn open(dir: &Path, backend: bool) -> (ViewerSession, Rc<RefCell<Calls>>) {
        let calls = Rc::new(RefCell::new(Calls::default()));
        let link = FakeLink(Rc::clone(&calls));
        let session = ViewerSession::open(
            Launch {
                project_root: dir.to_path_buf(),
                diagram: dir.join("diagram.edn"),
                restart: false,
                backend,
            },
            move || Box::new(link),
        )
        .unwrap();
        (session, calls)
    }

    fn at(start: Instant, ms: u64) -> Now {
        Now {
            at: start + std::time::Duration::from_millis(ms),
            epoch: 1_700_000_000,
        }
    }

    fn write_inbox(dir: &Path, text: &str) {
        std::fs::write(dir.join(".uml-viewer/to-viewer.edn"), text).unwrap();
    }

    fn context(state: &ViewState) -> Option<Value> {
        state
            .discussion_context
            .as_ref()
            .and_then(|c| get(c, "context"))
            .cloned()
    }

    #[test]
    fn queued_mail_is_applied_oldest_first_and_popped() {
        let dir = project("mail");
        std::fs::write(dir.join("second.edn"), SECOND).unwrap();
        let (mut session, _) = open(&dir, true);
        assert!(
            session.state().waiting,
            "a backend launch waits for :display"
        );
        write_inbox(
            &dir,
            "{:next-id 3 :queue [{:id 1 :op :context :context :proposal :proposal-id :p :name \"中文\"} {:id 2 :op :display :path \"second.edn\"}]}",
        );
        let effects = session.process(Event::MailDue, at(Instant::now(), 0));
        let state = session.state();
        assert_eq!(2, state.mail_seen);
        assert!(!state.waiting, ":display ends the waiting canvas");
        assert_eq!(
            Some(&Value::String("Second diagram".into())),
            state.document.as_ref().and_then(|d| get(d, "title"))
        );
        assert_eq!(Some(Value::keyword("proposal")), context(state));
        assert_eq!(
            "{:next-id 3 :queue []}",
            std::fs::read_to_string(dir.join(".uml-viewer/to-viewer.edn")).unwrap()
        );
        assert_eq!(
            Effects {
                repaint: Repaint::Now,
                invalidate_gui: true,
                close: None,
            },
            effects
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn idle_ticks_ask_for_no_repaint_and_a_changed_diagram_reloads_on_the_poll_cadence() {
        let dir = project("reload");
        std::fs::write(dir.join("diagram.edn"), "{:title \"First\"}").unwrap();
        let (mut session, _) = open(&dir, false);
        let title = |s: &ViewerSession| {
            s.state()
                .document
                .as_ref()
                .and_then(|d| get(d, "title"))
                .cloned()
        };
        let start = Instant::now();
        for ms in [0, 250, 500, 750] {
            assert_eq!(
                Effects::default(),
                session.process(Event::Tick, at(start, ms)),
                "an idle tick at {ms} ms must not repaint"
            );
        }
        assert_eq!(
            Effects::default(),
            session.process(Event::MailDue, at(start, 750)),
            "an idle mail poll must not repaint"
        );
        let revision = session.state().revision;
        // A later mtime: the stamp is the file's modification time.
        std::fs::write(dir.join("diagram.edn"), "{:title \"Second\"}").unwrap();
        let later = std::time::SystemTime::now() + std::time::Duration::from_secs(5);
        std::fs::File::options()
            .write(true)
            .open(dir.join("diagram.edn"))
            .unwrap()
            .set_modified(later)
            .unwrap();
        session.process(Event::Tick, at(start, 900));
        assert_eq!(
            Some(Value::String("First".into())),
            title(&session),
            "no reload before the poll interval elapsed"
        );
        session.process(Event::Tick, at(start, 1000));
        assert_eq!(Some(Value::String("Second".into())), title(&session));
        assert_eq!(revision + 1, session.state().revision);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_forced_refresh_reloads_on_the_next_tick_without_waiting_for_the_cadence() {
        let dir = project("refresh");
        std::fs::write(dir.join("diagram.edn"), "{:title \"First\"}").unwrap();
        let (mut session, _) = open(&dir, false);
        let start = Instant::now();
        session.process(Event::Tick, at(start, 0));
        std::fs::remove_file(dir.join("diagram.edn")).unwrap();
        session.process(Event::Tick, at(start, 10));
        assert_eq!(None, session.hint(), "the cadence has not elapsed");
        session.process(Event::ForceRefresh, at(start, 20));
        session.process(Event::Tick, at(start, 20));
        assert!(
            session.hint().is_some_and(|h| h.contains("diagram.edn")),
            "{:?}",
            session.hint()
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn one_mail_poll_applies_the_whole_queue_before_the_shell_sees_quit_for_restart() {
        let dir = project("restart");
        std::fs::write(dir.join("diagram.edn"), SECOND).unwrap();
        let (mut session, calls) = open(&dir, false);
        write_inbox(
            &dir,
            "{:next-id 3 :queue [{:id 1 :op :context :context :real} {:id 2 :op :quit-for-restart}]}",
        );
        session.process(Event::MailDue, at(Instant::now(), 0));
        let state = session.state();
        assert!(state.quit_for_restart);
        assert_eq!(Some(Value::keyword("real")), context(state));
        assert_eq!(2, state.mail_seen);
        assert!(
            calls.borrow().stops.is_empty(),
            "the restart exit, not the mail, stops the helper"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    fn dims() -> Dims {
        Dims {
            window_w: 1500.0,
            window_h: 920.0,
            ..Dims::default()
        }
    }

    #[test]
    fn the_right_click_menu_writes_the_exact_to_agent_contract_and_wakes_each_delivery() {
        let dir = project("menu");
        std::fs::write(dir.join("diagram.edn"), SECOND).unwrap();
        let (mut session, calls) = open(&dir, false);
        // The compiled scene id of the clicked class.
        session.state_mut().selected = Some(edn::parse("{:kind :class :id :d0-target}").unwrap());
        let start = Instant::now();
        for op in [
            "refresh-crap",
            "refresh-mutate",
            "refresh-mutate-all",
            "omit",
        ] {
            let effects =
                session.process(Event::Action(Action::Mail(op.into()), dims()), at(start, 0));
            assert_eq!(Repaint::Now, effects.repaint);
        }
        assert_eq!(
            "{:next-id 5 :queue [{:id 1 :op :refresh-crap :target {:id :target :kind :class :ns \"app.target\"}} {:id 2 :op :refresh-mutate :target {:id :target :kind :class :ns \"app.target\"}} {:id 3 :op :refresh-mutate-all :target {:id :target :kind :class :ns \"app.target\"}} {:id 4 :op :omit :target {:id :target :kind :class :ns \"app.target\"}}]}",
            std::fs::read_to_string(dir.join(".uml-viewer/to-agent.edn")).unwrap()
        );
        assert_eq!(vec![1, 2, 3, 4], calls.borrow().woken);
        assert_eq!(
            Effects::default(),
            session.process(Event::MailDue, at(start, 0)),
            "the viewer's own writes are not news to its mail poll"
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn regen_reports_whether_the_companion_was_woken_and_always_queues_the_request() {
        let dir = project("regen");
        std::fs::write(dir.join("diagram.edn"), SECOND).unwrap();
        let (mut session, calls) = open(&dir, false);
        let now = at(Instant::now(), 0);
        session.process(Event::Action(Action::Regen, dims()), now);
        assert_eq!(Some("Regen requested."), session.status());
        calls.borrow_mut().wake_fails = true;
        session.process(Event::Action(Action::Regen, dims()), now);
        assert_eq!(
            Some("Regen queued; Claude session not attached."),
            session.status()
        );
        assert_eq!(
            "{:next-id 3 :queue [{:id 1 :op :regen} {:id 2 :op :regen}]}",
            std::fs::read_to_string(dir.join(".uml-viewer/to-agent.edn")).unwrap()
        );
        assert_eq!(vec![1, 2], calls.borrow().woken);
        let _ = std::fs::remove_dir_all(dir);
    }

    fn chrome() -> Chrome {
        Chrome {
            window_w: Some(1100.0),
            window_h: Some(700.0),
            inspector_w: Some(340.0),
        }
    }

    #[test]
    fn the_restart_exit_keeps_the_companion_and_saves_the_snapshot_only_once_it_settled() {
        let dir = project("restart exit");
        std::fs::write(dir.join("diagram.edn"), SECOND).unwrap();
        let (mut session, calls) = open(&dir, false);
        let start = Instant::now();
        assert_eq!(
            Effects::default(),
            session.process(Event::RestartExit(chrome()), at(start, 0)),
            "without :quit-for-restart the checkpoint does nothing"
        );
        assert!(calls.borrow().stops.is_empty());
        calls.borrow_mut().unsettled = true;
        write_inbox(&dir, "{:next-id 2 :queue [{:id 1 :op :quit-for-restart}]}");
        session.process(Event::MailDue, at(start, 0));
        let saved = dir.join(".uml-viewer/session.edn");
        for ms in [0, 250, 500] {
            let effects = session.process(Event::RestartExit(chrome()), at(start, ms));
            assert_eq!(
                Effects {
                    repaint: Repaint::After(POLL_INTERVAL),
                    ..Effects::default()
                },
                effects,
                "a stopping helper is polled on the cadence at {ms} ms"
            );
            assert!(!saved.exists(), "no snapshot before the helper settled");
        }
        assert_eq!(
            vec![true],
            calls.borrow().stops,
            "one stop, keeping the companion"
        );
        assert!(calls.borrow().polls >= 3, "the stopping helper is polled");
        calls.borrow_mut().unsettled = false;
        let effects = session.process(Event::RestartExit(chrome()), at(start, 750));
        assert_eq!(Some(0), effects.close);
        let snapshot = edn::parse(&std::fs::read_to_string(&saved).unwrap()).unwrap();
        assert_eq!(Some(&Value::Integer(1100)), get(&snapshot, "window-w"));
        assert_eq!(
            Some(&Value::Integer(340)),
            get(&snapshot, "inspector-width")
        );
        assert_eq!(Some(&Value::Integer(1)), get(&snapshot, "mail-seen"));
        assert_eq!(vec![true], calls.borrow().stops);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_normal_close_stops_without_keeping_and_exits_at_the_five_second_deadline() {
        let dir = project("normal close");
        std::fs::write(dir.join("diagram.edn"), SECOND).unwrap();
        let (mut session, calls) = open(&dir, false);
        let start = Instant::now();
        assert_eq!(
            Effects::default(),
            session.process(Event::Quitting, at(start, 0)),
            "before a close the checkpoint does nothing"
        );
        calls.borrow_mut().unsettled = true;
        assert_eq!(
            Effects::default(),
            session.process(Event::Quit, at(start, 0))
        );
        session.process(Event::Quit, at(start, 100));
        assert_eq!(vec![false], calls.borrow().stops, "one stop, not keeping");
        for ms in [0, 1000, 4999] {
            assert_eq!(
                Effects {
                    repaint: Repaint::After(POLL_INTERVAL),
                    ..Effects::default()
                },
                session.process(Event::Quitting, at(start, ms)),
                "still waiting for the helper at {ms} ms"
            );
        }
        assert_eq!(
            Some(0),
            session.process(Event::Quitting, at(start, 5000)).close,
            "the deadline closes regardless"
        );
        assert!(!dir.join(".uml-viewer/session.edn").exists());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_normal_close_exits_as_soon_as_the_helper_settled() {
        let dir = project("settled close");
        std::fs::write(dir.join("diagram.edn"), SECOND).unwrap();
        let (mut session, calls) = open(&dir, false);
        let start = Instant::now();
        assert_eq!(
            Some(0),
            session.process(Event::Quit, at(start, 0)).close,
            "no helper to wait for"
        );
        let (mut session, calls2) = open(&dir, false);
        calls2.borrow_mut().unsettled = true;
        session.process(Event::Quit, at(start, 0));
        assert_eq!(None, session.process(Event::Quitting, at(start, 250)).close);
        calls2.borrow_mut().unsettled = false;
        assert_eq!(
            Some(0),
            session.process(Event::Quitting, at(start, 500)).close
        );
        assert_eq!(vec![false], calls.borrow().stops);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_closed_window_stops_an_unsettled_helper_without_keeping_it() {
        let dir = project("window closed");
        std::fs::write(dir.join("diagram.edn"), SECOND).unwrap();
        let (mut session, calls) = open(&dir, false);
        session.process(Event::WindowClosed, at(Instant::now(), 0));
        assert!(
            calls.borrow().stops.is_empty(),
            "a settled helper is left alone"
        );
        calls.borrow_mut().unsettled = true;
        session.process(Event::WindowClosed, at(Instant::now(), 0));
        assert_eq!(vec![false], calls.borrow().stops);
        let _ = std::fs::remove_dir_all(dir);
    }
}
