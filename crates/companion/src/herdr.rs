//! Herdr session backend (port of `love/uml/adapters/herdr.lua`). Every CLI
//! request selects the recorded session; the selected executable is fixed for
//! this process. Ownership is EDN on disk and EDN maps in memory.

use crate::herdr_domain as domain;
use crate::json::Json;
use crate::posix::{self, Failure, Owner, Result, RunOut};
use crate::record;
use crate::recovery;
use crate::text::shell_word;
use edn::Value;
use std::path::Path;
use std::rc::Rc;

/// The first `workspace create` waits out the server bootstrap; WSL PATH
/// entries under /mnt/c stretch it past the ordinary command bound (contract
/// §6).
const BOOTSTRAP_MS: i64 = 60000;

/// A dropped `pane run` is indistinguishable from a slow runner start, so
/// the submission is retried a bounded number of times before failing.
const ACK_ATTEMPTS: u32 = 3;
const ACK_WAIT_MS: i64 = 10000;

pub use crate::domain::Selected;

/// How the runner role is launched inside the pane: the argv the pane's shell
/// receives after quoting (the manifest path is appended per generation).
#[derive(Clone)]
pub struct RunnerLauncher {
    pub argv: Vec<String>,
}

/// How the detached controller is spawned (`spawn_detached` prepends the
/// executable).
#[derive(Clone)]
pub struct ControllerSpec {
    pub executable: String,
    pub argv: Vec<String>,
}

/// Injected boundaries. The backend owns every external command; these seams
/// let a test answer the Herdr protocol and drive the journal without a real
/// server or filesystem.
type RunFn = Rc<dyn Fn(&[String], Option<i64>) -> RunOut>;
type OwnerFn = Rc<dyn Fn(i64) -> Option<Owner>>;
type ClockFn = Rc<dyn Fn() -> i64>;
type SleepFn = Rc<dyn Fn(i64)>;
type CanonicalFn = Rc<dyn Fn(&str) -> String>;
type CompanionWrite = Rc<dyn Fn(&str, &Value) -> Result<()>>;
type CompanionRead = Rc<dyn Fn(&str) -> Result<Option<Value>>>;
/// (executable, argv, cwd, inherited variables to remove).
type ServerSpawn = Rc<dyn Fn(&str, &[String], &str, &[String]) -> Result<Option<Owner>>>;
/// Names of the variables this process would pass to a child.
type EnvironmentFn = Rc<dyn Fn() -> Vec<String>>;
type RunnerStateFn = Rc<dyn Fn(&Value) -> Option<Value>>;
type StartRecovery = Rc<dyn Fn(&Value, Option<&ControllerSpec>) -> Result<()>>;
type CancelRecovery = Rc<dyn Fn(&Value) -> Result<()>>;
/// Reports one create/relaunch stage. The worker uses it to publish a
/// progress event and to let a cancellation land between boundaries, so it
/// may refuse and stop the startup.
pub type ProgressFn = Rc<dyn Fn(&str) -> Result<()>>;
type SelectFn = Rc<dyn Fn() -> Result<Selected>>;

#[derive(Clone)]
pub struct Options {
    pub run: RunFn,
    pub owner: OwnerFn,
    pub clock: ClockFn,
    pub sleep: SleepFn,
    pub canonical: CanonicalFn,
    /// The selected Herdr for this process; every CLI call uses it.
    pub selected: Option<Selected>,
    pub write_companion: CompanionWrite,
    pub read_companion: CompanionRead,
    pub spawn_server: Option<ServerSpawn>,
    pub runner: Option<RunnerLauncher>,
    pub runner_state: Option<RunnerStateFn>,
    pub start_recovery: Option<StartRecovery>,
    pub cancel_recovery: Option<CancelRecovery>,
    pub recovery: Option<ControllerSpec>,
    pub progress: Option<ProgressFn>,
    pub select: Option<SelectFn>,
    pub max_socket_bytes: usize,
    pub environment: EnvironmentFn,
}

impl Options {
    /// The production host: real POSIX process boundary, ownership file and
    /// recovery adapter.
    pub fn real() -> Options {
        Options {
            run: Rc::new(|argv, timeout| {
                let argv: Vec<std::ffi::OsString> =
                    argv.iter().map(std::ffi::OsString::from).collect();
                posix::run(&argv, timeout, &[])
            }),
            owner: Rc::new(|pid| {
                if pid < 1 || pid > i32::MAX as i64 {
                    return None;
                }
                posix::identity(pid as i32).ok().flatten()
            }),
            clock: Rc::new(posix::monotonic_ms),
            sleep: Rc::new(posix::sleep),
            canonical: Rc::new(|path| posix::canonical(path).unwrap_or_else(|_| path.to_owned())),
            selected: None,
            write_companion: Rc::new(crate::ownership::write),
            read_companion: Rc::new(crate::ownership::read),
            spawn_server: None,
            runner: None,
            runner_state: None,
            start_recovery: None,
            cancel_recovery: None,
            recovery: None,
            progress: None,
            select: None,
            max_socket_bytes: 101,
            environment: Rc::new(|| {
                std::env::vars_os()
                    .map(|(name, _)| name.to_string_lossy().into_owned())
                    .collect()
            }),
        }
    }
}

pub struct Herdr {
    pub identity: Option<Value>,
    pub options: Options,
    selected: Option<Selected>,
}

fn strings(argv: &[&str]) -> Vec<String> {
    argv.iter().map(|s| (*s).to_owned()).collect()
}

impl Herdr {
    pub fn new(identity: Option<Value>, options: Options) -> Herdr {
        let selected = options.selected.clone();
        Herdr {
            identity,
            options,
            selected,
        }
    }

    pub fn herdr(&self) -> String {
        self.selected
            .as_ref()
            .map(|selected| selected.path.clone())
            .unwrap_or_else(|| "herdr".to_owned())
    }

    fn selected(&mut self) -> Result<Selected> {
        if let Some(selected) = &self.selected {
            return Ok(selected.clone());
        }
        if let Some(select) = &self.options.select {
            self.selected = Some(select()?);
            return Ok(self.selected.clone().unwrap());
        }
        Err(Failure::new("Herdr executable was not selected"))
    }

    fn fail<T>(&self, message: impl Into<String>) -> Result<T> {
        Err(Failure::new(message.into()))
    }

    fn checked(&self, argv: &[String], timeout_ms: Option<i64>) -> Result<RunOut> {
        let result = (self.options.run)(argv, timeout_ms);
        if result.timeout || result.exit != Some(0) {
            return self.fail(format!(
                "Companion command failed: {}: {}",
                argv[0], result.err
            ));
        }
        Ok(result)
    }

    fn parse(&self, result: &RunOut) -> Result<Json> {
        let value =
            crate::json::decode(&result.out).map_err(|_| Failure::new("Malformed Herdr JSON"))?;
        if !matches!(value, Json::Object(_)) {
            return self.fail("Malformed Herdr JSON");
        }
        if let Some(error) = value.get("error")
            && !matches!(error, Json::Null)
        {
            return self.fail("Herdr API error");
        }
        Ok(value)
    }

    fn cli_within(&self, timeout_ms: Option<i64>, session: &str, args: &[&str]) -> Result<Json> {
        let mut argv = vec![self.herdr(), "--session".to_owned(), session.to_owned()];
        argv.extend(args.iter().map(|s| (*s).to_owned()));
        self.parse(&self.checked(&argv, timeout_ms)?)
    }

    pub fn cli(&self, session: &str, args: &[&str]) -> Result<Json> {
        self.cli_within(None, session, args)
    }

    pub fn session_check(&self, session: &str) -> Result<()> {
        if !domain::session_valid(session) {
            return self.fail("Herdr requires an isolated uml-viewer named session");
        }
        Ok(())
    }

    pub fn status(&self, session: &str) -> Result<Json> {
        let value = self.cli(session, &["status", "server", "--json"])?;
        let socket = value.get("socket").and_then(Json::as_str);
        let valid = value.get("session").and_then(Json::as_str) == Some(session)
            && socket.is_some_and(|socket| socket.starts_with('/'))
            && matches!(value.get("running"), Some(Json::Bool(_)));
        if !valid {
            return self.fail("Ambiguous Herdr endpoint");
        }
        Ok(value)
    }

    pub fn compatible(&self, status: &Json) -> Result<()> {
        let ok = status.get("running") == Some(&Json::Bool(true))
            && status
                .get("version")
                .and_then(Json::as_str)
                .is_some_and(crate::domain::herdr_version_supported)
            && status.get("protocol").and_then(Json::as_i64) == Some(22)
            && status.get("compatible") == Some(&Json::Bool(true))
            && status.get("endpoint_compatible") == Some(&Json::Bool(true));
        if !ok {
            return self.fail("Herdr endpoint is not a verified >=0.9.3/protocol 22 server");
        }
        Ok(())
    }

    pub fn snapshot(&self, session: &str) -> Result<Json> {
        let data = self.cli(session, &["api", "snapshot"])?;
        let snapshot = data.get("result").and_then(|r| r.get("snapshot"));
        let valid = snapshot.is_some_and(|snapshot| {
            snapshot
                .get("version")
                .and_then(Json::as_str)
                .is_some_and(crate::domain::herdr_version_supported)
                && snapshot.get("protocol").and_then(Json::as_i64) == Some(22)
                && snapshot.get("panes").and_then(Json::array_items).is_some()
                && snapshot
                    .get("workspaces")
                    .and_then(Json::array_items)
                    .is_some()
        });
        match snapshot {
            Some(snapshot) if valid => Ok(snapshot.clone()),
            _ => self.fail("Invalid Herdr snapshot"),
        }
    }

    /// `/proc` identity of an owned process; the recorded pid is opaque text
    /// or an integer and must be a plain positive decimal.
    pub fn process_owner(&self, pid: &Value) -> Result<Option<Owner>> {
        let text = match pid {
            Value::Integer(n) => n.to_string(),
            Value::String(s) => s.clone(),
            _ => String::new(),
        };
        if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) || text.starts_with('0') {
            return self.fail("Invalid owned process PID");
        }
        let pid: i64 = text
            .parse()
            .map_err(|_| Failure::new("Invalid owned process PID"))?;
        Ok((self.options.owner)(pid))
    }

    pub fn owner_matches(&self, pid: &Value, start: &str) -> Result<Owner> {
        let observed = self.process_owner(pid)?;
        match observed {
            Some(observed) if observed.start == start => Ok(observed),
            _ => self.fail("Herdr process creation identity changed"),
        }
    }

    pub fn poll<T>(
        &self,
        timeout_ms: i64,
        interval_ms: i64,
        probe: &dyn Fn() -> Option<T>,
        message: &str,
    ) -> Result<T> {
        let deadline = (self.options.clock)() + timeout_ms;
        loop {
            if let Some(value) = probe() {
                return Ok(value);
            }
            if (self.options.clock)() < deadline {
                (self.options.sleep)(interval_ms);
            } else {
                return self.fail(message);
            }
        }
    }

    pub fn launch_server(&self, session: &str, cwd: &str) -> Result<Owner> {
        // Server diagnostics belong beside the project's other lifecycle
        // journals; without them a failed start is an unexplained timeout.
        let remove = domain::inherited_pane_env(&(self.options.environment)());
        let owner = match &self.options.spawn_server {
            Some(spawn) => spawn(
                &self.herdr(),
                &strings(&["--session", session, "server"]),
                cwd,
                &remove,
            )?,
            None => {
                let log = Path::new(cwd).join(".uml-viewer/herdr-server.log");
                let argv: Vec<std::ffi::OsString> = strings(&["--session", session, "server"])
                    .iter()
                    .map(std::ffi::OsString::from)
                    .collect();
                Some(posix::spawn_detached(
                    std::ffi::OsStr::new(&self.herdr()),
                    &argv,
                    Path::new(cwd),
                    Some(&log),
                    Some(Path::new(&format!("{}.err", log.display()))),
                    &remove.iter().map(String::as_str).collect::<Vec<_>>(),
                )?)
            }
        };
        owner.ok_or_else(|| Failure::new("Detached Herdr server exited"))
    }

    pub fn observe(&self, expected: &Value, session: &str, pane: &str) -> Result<()> {
        domain::complete(expected)?;
        if record::get_name(expected, "session").as_deref() != Some(session)
            || record::get_name(expected, "pane").as_deref() != Some(pane)
        {
            return self.fail("Herdr selector conflicts with ownership");
        }
        let status = self.status(session)?;
        self.compatible(&status)?;
        if status.get("socket").and_then(Json::as_str)
            != record::get_name(expected, "socket").as_deref()
        {
            return self.fail("Herdr socket changed");
        }
        let server_pid = edn::record::get(expected, "server-pid")
            .cloned()
            .unwrap_or(Value::Nil);
        let server_start = record::get_name(expected, "server-start").unwrap_or_default();
        self.owner_matches(&server_pid, &server_start)?;
        let snapshot = self.snapshot(session)?;
        let info = self.cli(session, &["pane", "process-info", "--pane", pane])?;
        let process_info = info.get("result").and_then(|r| r.get("process_info"));
        domain::observe(
            expected,
            &status,
            &snapshot,
            process_info,
            &*self.options.canonical,
        )?;
        let shell_pid = edn::record::get(expected, "shell-pid")
            .cloned()
            .unwrap_or(Value::Nil);
        let owner_start = record::get_name(expected, "owner-start").unwrap_or_default();
        self.owner_matches(&shell_pid, &owner_start)?;
        Ok(())
    }

    /// The runner's state file, parsed as EDN (or JSON for older records);
    /// only the current generation token counts.
    pub fn runner_state(&self, record: &Value) -> Option<Value> {
        if let Some(injected) = &self.options.runner_state {
            return injected(record);
        }
        let path = record::get_name(record, "runner-state")?;
        let text = std::fs::read_to_string(path).ok()?;
        let value = edn::parse(&text)
            .ok()
            .or_else(|| crate::json::decode(&text).ok().map(json_to_edn))?;
        if record::get_name(&value, "token") != record::get_name(record, "runner-token") {
            return None;
        }
        record::get_name(&value, "phase")?;
        Some(value)
    }

    /// The runner must exec the agent directly so its recorded identity is
    /// the agent's own process, never a shell that re-parses argv. Script
    /// wrappers are refused up front: an unreadable or PATH-missing
    /// executable cannot prove its child identity.
    pub fn require_direct_command(&self, command: &[String]) -> Result<String> {
        let first = command.first().cloned().unwrap_or_default();
        if first.is_empty() {
            return self.fail("Herdr command must be a nonempty argv vector");
        }
        let mut resolved = first.clone();
        if !first.contains('/') {
            for directory in std::env::var("PATH").unwrap_or_default().split(':') {
                let candidate = format!("{directory}/{first}");
                if std::fs::File::open(&candidate).is_ok() {
                    resolved = candidate;
                    break;
                }
            }
        }
        if std::fs::File::open(&resolved).is_err() {
            return self.fail(format!("Companion executable not found on PATH: {first}"));
        }
        Ok(resolved)
    }

    pub fn runner_command(&self, record: &Value, command: &[String]) -> Result<Vec<String>> {
        let first = self.require_direct_command(command)?;
        let command = if first != command[0] {
            let mut resolved = command.to_vec();
            resolved[0] = first;
            resolved
        } else {
            command.to_vec()
        };
        let state_path = record::get_name(record, "runner-state")
            .ok_or_else(|| Failure::new("Herdr ownership has no runner state path"))?;
        let manifest = format!("{state_path}.launch.edn");
        let mut copy = record.clone();
        edn::record::put(
            &mut copy,
            "command",
            Value::Vector(
                command
                    .iter()
                    .map(|arg| Value::String(arg.clone()))
                    .collect(),
            ),
        );
        recovery::write_state(Path::new(&manifest), &copy, &recovery::real_host())?;
        let runner = self
            .options
            .runner
            .as_ref()
            .ok_or_else(|| Failure::new("Herdr runner launcher is not configured"))?;
        let mut argv = runner.argv.clone();
        argv.push(manifest);
        Ok(argv)
    }

    /// `pane run` retries only canonical-mode rejections, within the ordinary
    /// command bound.
    pub fn pane_run(&self, session: &str, pane: &str, text: &str) -> Result<()> {
        let argv = vec![
            self.herdr(),
            "--session".to_owned(),
            session.to_owned(),
            "pane".to_owned(),
            "run".to_owned(),
            pane.to_owned(),
            text.to_owned(),
        ];
        let deadline = (self.options.clock)() + 10000;
        loop {
            let result = (self.options.run)(&argv, None);
            if result.exit == Some(0) && !result.timeout {
                return Ok(());
            }
            let output = format!("{}{}", result.out, result.err);
            if (self.options.clock)() < deadline && output.contains("canonical mode") {
                (self.options.sleep)(100);
            } else {
                return self.fail("Herdr pane run failed");
            }
        }
    }

    /// Reset what a previous agent left enabled. Mouse tracking modes go
    /// first: a pane still reporting SGR mouse events delivers them to the
    /// replacement agent as keystrokes. On POSIX the sequence is written to
    /// the pane's own terminal by running printf inside that pane.
    pub fn terminal_reset(&self, record: &Value) -> Result<()> {
        const RESET_SEQUENCE: &str = "\u{1b}[?1003l\u{1b}[?1002l\u{1b}[?1006l\u{1b}[?1000l\u{1b}[?1049l\u{1b}[?2004l\u{1b}[<u\u{1b}[<u\u{1b}[<u\u{1b}[?25h";
        let session = record::get_name(record, "session").unwrap_or_default();
        let pane = record::get_name(record, "pane").unwrap_or_default();
        self.pane_run(
            &session,
            &pane,
            &format!("printf %s {}", shell_word(RESET_SEQUENCE)),
        )?;
        (self.options.sleep)(250);
        self.checked(
            &[
                self.herdr(),
                "--session".to_owned(),
                session.clone(),
                "pane".to_owned(),
                "send-keys".to_owned(),
                pane,
                "ctrl+c".to_owned(),
            ],
            None,
        )?;
        (self.options.sleep)(150);
        Ok(())
    }

    /// Recovery relaunch: re-verify, reset the pane, submit the runner with
    /// the `--resume` recovery command on the same pane.
    pub fn relaunch(&mut self, record: &Value) -> Result<()> {
        let _ = self.selected()?;
        let session = record::get_name(record, "session").unwrap_or_default();
        let pane = record::get_name(record, "pane").unwrap_or_default();
        self.observe(record, &session, &pane)?;
        let info = self.cli(&session, &["pane", "process-info", "--pane", &pane])?;
        let process_info = info.get("result").and_then(|r| r.get("process_info"));
        let shell_pid = record::get_i64(record, "shell-pid").unwrap_or(0);
        domain::foreground_allowed(process_info, shell_pid)?;
        self.terminal_reset(record)?;
        let shell = self
            .process_owner(
                &edn::record::get(record, "shell-pid")
                    .cloned()
                    .unwrap_or(Value::Nil),
            )?
            .ok_or_else(|| Failure::new("Herdr process creation identity changed"))?;
        let command: Vec<String> = record::string_vec(
            record::get_vec(record, "recovery-command")
                .or_else(|| record::get_vec(record, "command")),
        );
        let argv = self.runner_command(record, &command)?;
        self.pane_run(&session, &pane, &domain::pane_command(&argv, &shell.name)?)
    }

    fn cancel_recovery(&self, record: &Value) -> Result<()> {
        if let Some(cancel) = &self.options.cancel_recovery {
            return cancel(record);
        }
        recovery::cancel(record, &recovery::real_host())
    }

    pub fn recovery_status(&self, record: &Value) -> Result<Option<Value>> {
        if edn::record::get(record, "supervisor").is_some() {
            return recovery::status(record, &recovery::real_host());
        }
        Ok(None)
    }

    /// Herdr derives its API socket from XDG_CONFIG_HOME and the session
    /// name. Measured against 0.9.5: a 101-byte path works and 102 fails, and
    /// the failure surfaces only later as a closed-channel error on workspace
    /// create, so refuse up front with the actual cause.
    pub fn socket_check(&self, session: &str) -> Result<()> {
        let root = std::env::var("XDG_CONFIG_HOME")
            .ok()
            .filter(|root| !root.is_empty())
            .unwrap_or_else(|| format!("{}/.config", std::env::var("HOME").unwrap_or_default()));
        let path = format!(
            "{}/herdr/sessions/{session}/herdr.sock",
            root.trim_end_matches('/')
        );
        let limit = self.options.max_socket_bytes;
        if path.len() > limit {
            return self.fail(format!(
                "Herdr socket path exceeds the Unix socket limit ({} > {limit}): {path}; set a shorter XDG_CONFIG_HOME or project path",
                path.len()
            ));
        }
        Ok(())
    }

    /// A fresh pane's shell is not ready for input the moment `workspace
    /// create` returns: the first `pane run` is discarded while the shell
    /// boots (measured on Herdr 0.9.5; it fails silently with exit 0). Wait
    /// for the pane's first output, bounded, before the first submission.
    /// `pane read` prints terminal text, not JSON, so it bypasses `cli`.
    fn await_pane_ready(&self, session: &str, pane: &str) -> Result<()> {
        let argv = vec![
            self.herdr(),
            "--session".to_owned(),
            session.to_owned(),
            "pane".to_owned(),
            "read".to_owned(),
            pane.to_owned(),
        ];
        self.poll(
            15000,
            100,
            &|| {
                let result = (self.options.run)(&argv, None);
                (result.exit == Some(0) && !result.timeout && !result.out.trim().is_empty())
                    .then_some(())
            },
            "Herdr pane shell never became ready",
        )?;
        Ok(())
    }

    pub fn create(&mut self, session: &str, cwd: &str, command: &[String]) -> Result<Value> {
        self.session_check(session)?;
        self.socket_check(session)?;
        if command.is_empty() {
            return self.fail("Herdr command must be a nonempty argv vector");
        }
        let resolved = self.require_direct_command(command)?;
        let command = if resolved != command[0] {
            let mut command = command.to_vec();
            command[0] = resolved;
            command
        } else {
            command.to_vec()
        };
        if self
            .identity
            .as_ref()
            .is_some_and(|identity| record::get_bool(identity, "provisional") != Some(true))
        {
            return self.fail("Cannot create over existing Herdr ownership");
        }
        let sessions = self.cli(session, &["session", "list", "--json"])?;
        let inventory = sessions
            .get("sessions")
            .and_then(Json::array_items)
            .ok_or_else(|| Failure::new("Ambiguous session inventory"))?;
        for entry in inventory {
            if entry.get("name").and_then(Json::as_str) == Some(session) {
                return self.fail("Herdr named session already exists");
            }
        }
        let canonical_cwd = (self.options.canonical)(cwd);
        let mut base =
            domain::ownership_base(self.identity.as_ref(), session, &canonical_cwd, &command);
        (self.options.write_companion)(&canonical_cwd, &base)?;
        self.progress("server")?;
        let owner = self.launch_server(session, &canonical_cwd)?;
        edn::record::put(
            &mut base,
            "server-pid",
            Value::String(owner.pid.to_string()),
        );
        edn::record::put(
            &mut base,
            "server-start",
            Value::String(owner.start.clone()),
        );
        (self.options.write_companion)(&canonical_cwd, &base)?;
        let status = self.poll(
            5000,
            50,
            &|| match self.status(session) {
                Ok(value) if value.get("running") == Some(&Json::Bool(true)) => Some(value),
                _ => None,
            },
            "Herdr server did not become reachable",
        )?;
        self.compatible(&status)?;
        edn::record::put(
            &mut base,
            "socket",
            Value::String(
                status
                    .get("socket")
                    .and_then(Json::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            ),
        );
        (self.options.write_companion)(&canonical_cwd, &base)?;
        self.progress("workspace")?;
        let created = self.cli_within(
            Some(BOOTSTRAP_MS),
            session,
            &["workspace", "create", "--cwd", &canonical_cwd, "--no-focus"],
        )?;
        let result = created
            .get("result")
            .ok_or_else(|| Failure::new("Malformed Herdr JSON"))?;
        let pane = result
            .get("root_pane")
            .ok_or_else(|| Failure::new("Malformed Herdr JSON"))?;
        let workspace = result
            .get("workspace")
            .and_then(|w| w.get("workspace_id"))
            .and_then(Json::as_str)
            .unwrap_or_default()
            .to_owned();
        edn::record::put(&mut base, "workspace", Value::String(workspace));
        edn::record::put(
            &mut base,
            "pane",
            Value::String(
                pane.get("pane_id")
                    .and_then(Json::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            ),
        );
        edn::record::put(
            &mut base,
            "terminal-id",
            Value::String(
                pane.get("terminal_id")
                    .and_then(Json::as_str)
                    .unwrap_or_default()
                    .to_owned(),
            ),
        );
        (self.options.write_companion)(&canonical_cwd, &base)?;
        let pane_id = record::get_name(&base, "pane").unwrap_or_default();
        let info = self.cli(session, &["pane", "process-info", "--pane", &pane_id])?;
        let process_info = info
            .get("result")
            .and_then(|r| r.get("process_info"))
            .cloned()
            .ok_or_else(|| Failure::new("Malformed Herdr JSON"))?;
        let shell_pid = process_info
            .get("shell_pid")
            .and_then(Json::as_i64)
            .ok_or_else(|| Failure::new("Malformed Herdr JSON"))?;
        let shell = self
            .process_owner(&Value::Integer(shell_pid))?
            .ok_or_else(|| Failure::new("Herdr process creation identity changed"))?;
        edn::record::put(&mut base, "shell-pid", Value::Integer(shell_pid));
        edn::record::put(&mut base, "owner-start", Value::String(shell.start.clone()));
        edn::record::put(
            &mut base,
            "runner-token",
            Value::String(recovery::new_token()),
        );
        let root = format!("{canonical_cwd}/.uml-viewer/");
        let runner_state = format!("{root}{session}-runner.edn");
        edn::record::put(&mut base, "runner-state", Value::String(runner_state));
        edn::record::put(
            &mut base,
            "supervisor",
            edn::record::record(vec![
                ("token", Value::String(recovery::new_token())),
                (
                    "manifest",
                    Value::String(format!("{root}recovery-launch.edn")),
                ),
                ("state-file", Value::String(format!("{root}recovery.edn"))),
                (
                    "control-file",
                    Value::String(format!("{root}recovery-control.edn")),
                ),
                (
                    "gate-file",
                    Value::String(format!("{root}recovery-gate.lock")),
                ),
                (
                    "lock-file",
                    Value::String(format!("{root}recovery-controller.lock")),
                ),
            ]),
        );
        (self.options.write_companion)(&canonical_cwd, &base)?;
        self.observe(&base, session, &pane_id)?;
        self.progress("runner")?;
        self.await_pane_ready(session, &pane_id)?;
        let argv = self.runner_command(&base, &command)?;
        let submission = domain::pane_command(&argv, &shell.name)?;
        // Herdr 0.9.5 can still swallow the submission after the pane has
        // printed its first output, and it reports success when it does.
        // Resubmit while no runner state appears: the runner takes a
        // generation lock, so a duplicate simply exits and cannot run twice.
        let mut acknowledged = None;
        for attempt in 0..ACK_ATTEMPTS {
            self.pane_run(session, &pane_id, &submission)?;
            match self.poll(
                ACK_WAIT_MS,
                100,
                &|| self.runner_state(&base),
                "Herdr runner did not acknowledge startup",
            ) {
                Ok(state) => {
                    acknowledged = Some(state);
                    break;
                }
                Err(error) => {
                    if attempt + 1 == ACK_ATTEMPTS {
                        return Err(error);
                    }
                }
            }
        }
        let state = acknowledged.ok_or_else(|| Failure::new("No runner acknowledgement"))?;
        let phase = record::get_name(&state, "phase").unwrap_or_default();
        let clean_exit = phase == "exited" && record::get_i64(&state, "exit") == Some(0);
        if phase != "started" && !clean_exit {
            return self.fail("Herdr runner failed during startup");
        }
        self.observe(&base, session, &pane_id)?;
        let owned = domain::committed(&base);
        (self.options.write_companion)(&canonical_cwd, &owned)?;
        self.progress("recovery")?;
        let start_recovery = self.options.start_recovery.clone();
        match start_recovery {
            Some(start) => start(&owned, self.options.recovery.as_ref())?,
            None => {
                recovery::start(&owned, &recovery::real_host(), &|identity, manifest| {
                    let controller = self.options.recovery.as_ref().ok_or_else(|| {
                        Failure::new("A detached controller launcher is required")
                    })?;
                    let mut argv: Vec<std::ffi::OsString> = controller
                        .argv
                        .iter()
                        .map(std::ffi::OsString::from)
                        .collect();
                    argv.push(std::ffi::OsString::from(manifest));
                    let log = crate::recovery_domain::state_file(identity)
                        .replace("recovery.edn", "recovery.log");
                    posix::spawn_detached(
                        std::ffi::OsStr::new(&controller.executable),
                        &argv,
                        Path::new(&canonical_cwd),
                        Some(Path::new(&log)),
                        Some(Path::new(&format!("{log}.err"))),
                        &[],
                    )?;
                    Ok(())
                })?;
            }
        }
        self.progress("committed")?;
        self.identity = Some(owned.clone());
        Ok(owned)
    }

    pub fn identity_observe(&self, session: &str, pane: &str) -> Result<()> {
        let identity = self
            .identity
            .as_ref()
            .ok_or_else(|| Failure::new("Incomplete Herdr ownership"))?;
        self.observe(&identity.clone(), session, pane)
    }

    pub fn probe(&self, session: &str) -> Result<bool> {
        let identity = self
            .identity
            .as_ref()
            .ok_or_else(|| Failure::new("Incomplete Herdr ownership"))?
            .clone();
        let pane = record::get_name(&identity, "pane").unwrap_or_default();
        self.observe(&identity, session, &pane)?;
        Ok(true)
    }

    pub fn bind(&self, session: &str, pane: &str) -> Result<String> {
        let identity = self
            .identity
            .as_ref()
            .ok_or_else(|| Failure::new("Incomplete Herdr ownership"))?
            .clone();
        self.observe(&identity, session, pane)?;
        Ok(pane.to_owned())
    }

    /// Wake delivers literal text carrying this delivery's unique token and
    /// returns only after consumption is acknowledged (contract §5).
    pub fn wake(
        &self,
        pane: &str,
        keys: &[String],
        token: &str,
        acknowledged: &dyn Fn(&str) -> bool,
        timeout_ms: Option<i64>,
        interval_ms: Option<i64>,
    ) -> Result<()> {
        domain::wake_arguments(token, keys)?;
        let identity = self
            .identity
            .as_ref()
            .ok_or_else(|| Failure::new("Incomplete Herdr ownership"))?
            .clone();
        let session = record::get_name(&identity, "session").unwrap_or_default();
        self.observe(&identity, &session, pane)?;
        if acknowledged(token) {
            return self.fail("Herdr wake acknowledgement predates this delivery");
        }
        let snapshot = self.snapshot(&session)?;
        let terminal_id = record::get_name(&identity, "terminal-id").unwrap_or_default();
        let occupied = snapshot
            .get("agents")
            .and_then(Json::array_items)
            .is_some_and(|agents| {
                agents.iter().any(|agent| {
                    agent.get("pane_id").and_then(Json::as_str) == Some(pane)
                        && agent.get("terminal_id").and_then(Json::as_str)
                            == Some(terminal_id.as_str())
                        && agent.get("agent").and_then(Json::as_str) == Some("claude")
                })
            });
        if !occupied {
            return self.fail("No Claude agent occupies the owned Herdr pane");
        }
        self.checked(
            &[
                self.herdr(),
                "--session".to_owned(),
                session.clone(),
                "agent".to_owned(),
                "prompt".to_owned(),
                pane.to_owned(),
                keys[1].clone(),
            ],
            None,
        )?;
        self.poll(
            timeout_ms.unwrap_or(5000),
            interval_ms.unwrap_or(50),
            &|| acknowledged(token).then_some(()),
            "Herdr wake consumption was not acknowledged; mail remains queued",
        )?;
        self.observe(&identity, &session, pane)?;
        Ok(())
    }

    /// Recovery toggling: enable only verifies the existing durable
    /// supervisor; disable cancels it. Without a supervisor there is nothing
    /// to adopt and no replacement is ever invented.
    pub fn recovery(&self, session: &str, pane: Option<&str>, enabled: bool) -> Result<bool> {
        let identity = self
            .identity
            .as_ref()
            .ok_or_else(|| Failure::new("Incomplete Herdr ownership"))?
            .clone();
        let pane = pane
            .map(str::to_owned)
            .or_else(|| record::get_name(&identity, "pane"))
            .unwrap_or_default();
        self.observe(&identity, session, &pane)?;
        if edn::record::get(&identity, "supervisor").is_some() {
            if enabled {
                recovery::verify(&identity, &recovery::real_host())?;
                return Ok(true);
            }
            self.cancel_recovery(&identity)?;
            return Ok(false);
        }
        if enabled {
            return self.fail("No durable supervisor registered; refusing replacement");
        }
        Ok(false)
    }

    /// Cleanup: cancel recovery first, then destroy only the recorded
    /// resources, checkpointing before each destructive step so an interrupted
    /// cleanup resumes where it stopped (contract §2, §6).
    pub fn cleanup(&self, session: &str) -> Result<()> {
        let record = self
            .identity
            .as_ref()
            .ok_or_else(|| Failure::new("Incomplete Herdr ownership"))?
            .clone();
        domain::complete(&record)?;
        if session != record::get_name(&record, "session").unwrap_or_default() {
            return self.fail("Cleanup selector conflicts with ownership");
        }
        if edn::record::get(&record, "supervisor").is_some() {
            self.cancel_recovery(&record)?;
        }
        let cwd = record::get_name(&record, "cwd").unwrap_or_default();
        let saved = (self.options.read_companion)(&cwd)?;
        if let Some(saved) = &saved
            && !record::is_empty_map(saved)
            && domain::ownership_changed(&record, saved)
        {
            return self.fail("Cleanup ownership changed");
        }
        let checkpoint = |record: &Value, phase: &str| -> Result<()> {
            let mut value = record.clone();
            edn::record::put(&mut value, "cleanup-phase", Value::keyword(phase));
            (self.options.write_companion)(&cwd, &value)
        };
        let mut phase = saved
            .as_ref()
            .and_then(|saved| record::get_name(saved, "cleanup-phase"))
            .or_else(|| record::get_name(&record, "cleanup-phase"));
        let mut endpoint = self.status(session)?;
        if endpoint.get("socket").and_then(Json::as_str)
            != record::get_name(&record, "socket").as_deref()
        {
            return self.fail("Cleanup socket changed");
        }
        if phase.is_none() {
            self.observe(
                &record,
                session,
                &record::get_name(&record, "pane").unwrap_or_default(),
            )?;
            checkpoint(&record, "workspace-closing")?;
            phase = Some("workspace-closing".to_owned());
        }
        if matches!(
            phase.as_deref(),
            Some("workspace-closing") | Some("workspace-closed")
        ) {
            self.compatible(&endpoint)?;
            let server_pid = edn::record::get(&record, "server-pid")
                .cloned()
                .unwrap_or(Value::Nil);
            let server_start = record::get_name(&record, "server-start").unwrap_or_default();
            self.owner_matches(&server_pid, &server_start)?;
            let snapshot = self.snapshot(session)?;
            let workspace = record::get_name(&record, "workspace").unwrap_or_default();
            let present = snapshot
                .get("workspaces")
                .and_then(Json::array_items)
                .is_some_and(|workspaces| {
                    workspaces.iter().any(|w| {
                        w.get("workspace_id").and_then(Json::as_str) == Some(workspace.as_str())
                    })
                });
            if present {
                self.observe(
                    &record,
                    session,
                    &record::get_name(&record, "pane").unwrap_or_default(),
                )?;
                self.cli(session, &["workspace", "close", &workspace])?;
            }
            let snapshot = self.snapshot(session)?;
            let remains = snapshot
                .get("workspaces")
                .and_then(Json::array_items)
                .is_some_and(|workspaces| {
                    workspaces.iter().any(|w| {
                        w.get("workspace_id").and_then(Json::as_str) == Some(workspace.as_str())
                    })
                });
            if remains {
                return self.fail("Herdr workspace remains after close");
            }
            checkpoint(&record, "workspace-closed")?;
            phase = Some("workspace-closed".to_owned());
        }
        if !domain::cleanup_phase_valid(phase.as_deref()) {
            return self.fail("Unknown cleanup phase");
        }
        if !matches!(
            phase.as_deref(),
            Some("server-stopped") | Some("session-deleting")
        ) {
            checkpoint(&record, "server-stopping")?;
            endpoint = self.status(session)?;
            if endpoint.get("socket").and_then(Json::as_str)
                != record::get_name(&record, "socket").as_deref()
            {
                return self.fail("Cleanup socket changed");
            }
            let server_pid = edn::record::get(&record, "server-pid")
                .cloned()
                .unwrap_or(Value::Nil);
            let server_start = record::get_name(&record, "server-start").unwrap_or_default();
            if endpoint.get("running") == Some(&Json::Bool(true)) {
                self.compatible(&endpoint)?;
                self.owner_matches(&server_pid, &server_start)?;
                let stopped = self.cli(session, &["session", "stop", session, "--json"])?;
                let confirmed = stopped.get("stopped") == Some(&Json::Bool(true))
                    && stopped
                        .get("session")
                        .and_then(|s| s.get("name"))
                        .and_then(Json::as_str)
                        == Some(session)
                    && stopped
                        .get("session")
                        .and_then(|s| s.get("socket_path"))
                        .and_then(Json::as_str)
                        == record::get_name(&record, "socket").as_deref();
                if !confirmed {
                    return self.fail("Herdr session stop was not confirmed");
                }
            } else if self.process_owner(&server_pid)?.is_some() {
                return self.fail("Stopped endpoint has a live or reused server PID");
            }
            self.poll(
                3000,
                50,
                &|| match self.process_owner(&server_pid) {
                    Ok(None) => Some(()),
                    _ => None,
                },
                "Herdr server process remains after stop",
            )?;
            checkpoint(&record, "server-stopped")?;
        }
        endpoint = self.status(session)?;
        let server_pid = edn::record::get(&record, "server-pid")
            .cloned()
            .unwrap_or(Value::Nil);
        if !(endpoint.get("running") == Some(&Json::Bool(false))
            && endpoint.get("socket").and_then(Json::as_str)
                == record::get_name(&record, "socket").as_deref()
            && self.process_owner(&server_pid)?.is_none())
        {
            return self.fail("Herdr endpoint remains after stop");
        }
        checkpoint(&record, "session-deleting")?;
        let sessions = self.cli(session, &["session", "list", "--json"])?;
        let inventory = sessions
            .get("sessions")
            .and_then(Json::array_items)
            .ok_or_else(|| Failure::new("Ambiguous cleanup inventory"))?;
        for entry in inventory {
            if entry.get("name").and_then(Json::as_str) == Some(session) {
                let deleted = self.cli(session, &["session", "delete", session, "--json"])?;
                let confirmed = deleted.get("deleted") == Some(&Json::Bool(true))
                    && deleted
                        .get("session")
                        .and_then(|s| s.get("name"))
                        .and_then(Json::as_str)
                        == Some(session)
                    && deleted
                        .get("session")
                        .and_then(|s| s.get("socket_path"))
                        .and_then(Json::as_str)
                        == record::get_name(&record, "socket").as_deref();
                if !confirmed {
                    return self.fail("Herdr session deletion was not confirmed");
                }
                break;
            }
        }
        let sessions = self.cli(session, &["session", "list", "--json"])?;
        let inventory = sessions
            .get("sessions")
            .and_then(Json::array_items)
            .ok_or_else(|| Failure::new("Ambiguous cleanup inventory"))?;
        for entry in inventory {
            if entry.get("name").and_then(Json::as_str) == Some(session) {
                return self.fail("Herdr session remains after deletion");
            }
        }
        Ok(())
    }

    fn progress(&self, stage: &str) -> Result<()> {
        match &self.options.progress {
            Some(progress) => progress(stage),
            None => Ok(()),
        }
    }
}

/// Older runner side files were JSON; convert what the journal readers need.
fn json_to_edn(value: Json) -> Value {
    match value {
        Json::Null => Value::Nil,
        Json::Bool(b) => Value::Bool(b),
        Json::Number(n) if n.fract() == 0.0 && n.abs() <= 9.007_199_254_740_991e15 => {
            Value::Integer(n as i64)
        }
        Json::Number(n) => Value::Float(n),
        Json::String(s) => Value::String(s),
        Json::Array(items) => Value::Vector(items.into_iter().map(json_to_edn).collect()),
        Json::Object(map) => Value::Map(
            map.into_iter()
                .map(|(key, value)| (Value::keyword(&key.replace('_', "-")), json_to_edn(value)))
                .collect(),
        ),
    }
}

#[cfg(test)]
mod tests;
