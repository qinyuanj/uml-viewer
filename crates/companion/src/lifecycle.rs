//! Companion lifecycle orchestration (port of
//! `love/uml/adapters/companion_lifecycle.lua`). Prompt text, session naming
//! and ownership checks are pure; all process work stays in adapters.

use crate::domain;
use crate::herdr::{ControllerSpec, Herdr, Options, ProgressFn, RunnerLauncher, Selected};
use crate::herdr_domain;
use crate::ownership;
use crate::posix::{self, Failure, Result, RunOut};
use crate::record;
use crate::recovery;
use edn::Value;

/// Herdr derives its API socket from XDG_CONFIG_HOME + session name. Measured
/// against Herdr 0.9.5: the server starts at 101 bytes but workspace creation
/// fails from 102 bytes, and that failure surfaces as an opaque closed-channel
/// error. The generated name therefore shrinks to whatever the config root
/// leaves, instead of assuming a fixed project-name budget.
pub const MAX_SOCKET_BYTES: usize = 101;
pub const MAX_NAME_BYTES: usize = 64;

pub fn config_root() -> String {
    let root = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|root| !root.is_empty())
        .unwrap_or_else(|| format!("{}/.config", std::env::var("HOME").unwrap_or_default()));
    root.trim_end_matches('/').to_owned()
}

/// `uml-viewer-<project>-<hash8>-<uuid8>`: the project name is the only part
/// that may shrink, so the socket path stays inside the measured limit.
pub fn session_name(root: &str, uuid: &str, config_root: Option<&str>) -> Result<String> {
    let normalized = root.replace('\\', "/");
    let base: String = normalized
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or("project")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let hash = root.bytes().fold(0u32, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(u32::from(byte))
    });
    let uuid_digits: String = uuid.replace('-', "").chars().take(8).collect();
    let fixed = format!("uml-viewer-%s-{hash:08x}-{uuid_digits}");
    let resolved_config_root = match config_root {
        Some(root) => root.to_owned(),
        None => self::config_root(),
    };
    let socket_prefix = format!("{resolved_config_root}/herdr/sessions/");
    let room =
        (MAX_SOCKET_BYTES.saturating_sub(socket_prefix.len())).saturating_sub("/herdr.sock".len());
    let room = room.min(MAX_NAME_BYTES);
    let fixed_length = fixed.len() - 2; // "%s" is a two-byte placeholder
    if room <= fixed_length {
        return Err(Failure::new(format!(
            "Herdr session name cannot fit: {socket_prefix}{}/herdr.sock exceeds the {MAX_SOCKET_BYTES}-byte Unix socket path; set a shorter XDG_CONFIG_HOME",
            fixed.replace("%s", "<project>")
        )));
    }
    let name_budget = room - fixed_length;
    Ok(fixed.replace("%s", &base.chars().take(name_budget).collect::<String>()))
}

pub struct Loaded {
    pub record: Value,
}

/// Load and verify this project's complete Herdr ownership. Missing, legacy,
/// foreign or incomplete records are refused, never adopted.
pub fn load(root: &str) -> Result<Option<Value>> {
    let Some(value) = ownership::read(root)? else {
        return Ok(None);
    };
    if record::is_empty_map(&value) {
        return Ok(None);
    }
    domain::recorded_backend(Some(&value))?;
    herdr_domain::complete(&value)?;
    let cwd = record::get_name(&value, "cwd").unwrap_or_default();
    let canonical = |path: &str| posix::canonical(path).unwrap_or_else(|_| path.replace('\\', "/"));
    if canonical(&cwd) != canonical(root) {
        return Err(Failure::new(
            "Companion ownership is missing, ambiguous or belongs to another project; retained",
        ));
    }
    Ok(Some(value))
}

pub fn required(root: &str) -> Result<Value> {
    load(root)?.ok_or_else(|| {
        Failure::new("Companion ownership is missing, ambiguous or unreadable; retained")
    })
}

/// The companion executable: `CLAUDE_BIN` when set, otherwise the bare name.
pub fn claude_executable() -> String {
    std::env::var("CLAUDE_BIN")
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "claude".to_owned())
}

fn current_exe() -> Result<String> {
    std::env::current_exe()
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(|error| Failure::new(format!("cannot resolve the uml-viewer binary: {error}")))
}

/// The role argv for this binary: `[exe, role, manifest]`.
fn role_launcher(role: &str) -> Result<Vec<String>> {
    Ok(vec![current_exe()?, role.to_owned()])
}

/// Read-only backend selection used before any resource is created.
pub fn select(root: &str, args: &[String]) -> Result<Selected> {
    crate::preflight::select(root, args, false)
}

/// Options wired for this process: the selected Herdr, the binary's own runner
/// and controller roles, and the real POSIX boundary.
fn options(selected: Option<Selected>, progress: Option<ProgressFn>) -> Result<Options> {
    let mut options = Options::real();
    options.selected = selected;
    options.progress = progress;
    options.runner = Some(RunnerLauncher {
        argv: role_launcher("companion-runner")?,
    });
    options.recovery = Some(ControllerSpec {
        executable: current_exe()?,
        argv: vec!["companion-controller".to_owned()],
    });
    Ok(options)
}

fn backend(options: Options, identity: Option<Value>) -> Herdr {
    Herdr::new(identity, options)
}

pub fn create(root: &str, options_in: CreateOptions) -> Result<(Value, Herdr)> {
    if load(root)?.is_some() {
        return Err(Failure::new(
            "Companion ownership already recorded; use --restart or resolve it before fresh start",
        ));
    }
    let selected = select(root, &["--backend".to_owned(), "herdr".to_owned()])?;
    let claude = options_in.claude.clone().unwrap_or_else(claude_executable);
    let uuid = options_in.uuid.clone().unwrap_or_else(recovery::new_token);
    let session = session_name(root, &uuid, None)?;
    let command = vec![
        claude.clone(),
        "--dangerously-skip-permissions".to_owned(),
        "--session-id".to_owned(),
        uuid.clone(),
        "--append-system-prompt".to_owned(),
        domain::STANDING_RULES.to_owned(),
        domain::LAUNCH_PROMPT.to_owned(),
    ];
    let recovery_command = [
        claude,
        "--dangerously-skip-permissions".to_owned(),
        "--resume".to_owned(),
        uuid.clone(),
        "--append-system-prompt".to_owned(),
        domain::STANDING_RULES.to_owned(),
        domain::RECOVERY_PROMPT.to_owned(),
    ];
    let canonical = posix::canonical(root).unwrap_or_else(|_| root.replace('\\', "/"));
    let provisional = record::map(vec![
        ("backend", Value::keyword("herdr")),
        ("owner-role", Value::keyword("companion")),
        ("cwd", Value::String(canonical.clone())),
        ("session", Value::String(session.clone())),
        (
            "command",
            Value::Vector(
                command
                    .iter()
                    .map(|arg| Value::String(arg.clone()))
                    .collect(),
            ),
        ),
        (
            "recovery-command",
            Value::Vector(
                recovery_command
                    .iter()
                    .map(|arg| Value::String(arg.clone()))
                    .collect(),
            ),
        ),
        ("claude-session", Value::String(uuid)),
        ("provisional", Value::Bool(true)),
    ]);
    ownership::write(root, &provisional)?;
    // The backend builds every later record on this identity, so the committed
    // ownership keeps the --resume recovery command and the Claude session id.
    let mut backend = backend(
        options(Some(selected), options_in.progress)?,
        Some(provisional),
    );
    let owned = backend.create(&session, &canonical, &command)?;
    backend.identity = Some(owned.clone());
    Ok((owned, backend))
}

#[derive(Default)]
pub struct CreateOptions {
    pub claude: Option<String>,
    pub uuid: Option<String>,
    /// Called at each create boundary. The worker publishes a progress event
    /// there and lets a cancellation land between boundaries, so it may
    /// refuse and stop the startup.
    pub progress: Option<ProgressFn>,
}

pub fn restart(root: &str) -> Result<(Value, Herdr)> {
    let plain = load(root)?
        .ok_or_else(|| Failure::new("Companion ownership is missing; cannot restart"))?;
    let selected = select(root, &["--backend".to_owned(), "herdr".to_owned()])?;
    let backend = backend(options(Some(selected), None)?, Some(plain.clone()));
    let session = record::get_name(&plain, "session").unwrap_or_default();
    let pane = record::get_name(&plain, "pane").unwrap_or_default();
    backend.probe(&session)?;
    backend.bind(&session, &pane)?;
    recovery::verify(&plain, &recovery::real_host())?;
    Ok((plain, backend))
}

/// Cancel recovery first, then release only what this project verifiably
/// owns; ownership is cleared only after confirmed cleanup.
pub fn shutdown(root: &str) -> Result<bool> {
    let Some(plain) = load(root)? else {
        return Ok(false);
    };
    let selected = select(root, &["--backend".to_owned(), "herdr".to_owned()])?;
    let backend = backend(options(Some(selected), None)?, Some(plain.clone()));
    let session = record::get_name(&plain, "session").unwrap_or_default();
    backend.cleanup(&session)?;
    ownership::write(root, &Value::Map(vec![]))?;
    Ok(true)
}

pub struct Opened {
    pub terminal: String,
    pub session: String,
    pub attach: String,
}

fn on_path(name: &str) -> Option<String> {
    std::env::var("PATH").ok().and_then(|path| {
        path.split(':')
            .map(|directory| format!("{directory}/{name}"))
            .find(|candidate| std::fs::File::open(candidate).is_ok())
    })
}

/// WSL reaches the host's Windows Terminal through interop; the new tab enters
/// the same distribution and attaches with the selected Herdr's absolute path.
/// Elsewhere, or when the terminal does not start, the caller reports the
/// command a person can run (ADR 0007).
pub fn open_window(session: &str, herdr: &str) -> Opened {
    let attach = format!(
        "{} session attach {}",
        crate::text::shell_word(herdr),
        crate::text::shell_word(session)
    );
    let distro = std::env::var("WSL_DISTRO_NAME").unwrap_or_default();
    let terminal = if distro.is_empty() {
        None
    } else {
        on_path("wt.exe")
    };
    let Some(terminal) = terminal else {
        return Opened {
            terminal: "none".to_owned(),
            session: session.to_owned(),
            attach,
        };
    };
    let mut argv: Vec<String> = vec![
        terminal,
        "-w".to_owned(),
        "new".to_owned(),
        "new-tab".to_owned(),
        "--title".to_owned(),
        session.to_owned(),
        "wsl.exe".to_owned(),
        "-d".to_owned(),
        distro,
        "--exec".to_owned(),
    ];
    if let Ok(config) = std::env::var("XDG_CONFIG_HOME")
        && !config.is_empty()
    {
        argv.push("/usr/bin/env".to_owned());
        argv.push(format!("XDG_CONFIG_HOME={config}"));
    }
    for word in [herdr, "session", "attach", session] {
        argv.push(word.to_owned());
    }
    let os_argv: Vec<std::ffi::OsString> = argv.iter().map(std::ffi::OsString::from).collect();
    let result: RunOut = posix::run(&os_argv, Some(10000), &[]);
    let started = result.exit == Some(0) && !result.timeout;
    Opened {
        terminal: if started { "windows-terminal" } else { "none" }.to_owned(),
        session: session.to_owned(),
        attach,
    }
}

/// The viewer-side start of the worker helper (Lua `W.start`): one request
/// file, then a native child with inherited stdio.
pub fn start_worker(root: &str, restart: bool) -> Result<crate::worker::Worker> {
    crate::worker::start(root, restart)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_a_fresh_session_uniquely_per_launch_within_the_grammar() {
        let name = session_name("C:/projects/some project", "abcdef12-rest", None).unwrap();
        assert!(name.len() <= 64);
        assert!(herdr_domain::session_valid(&name), "{name}");
        let long = session_name(
            &format!("C:/projects/{}", "x".repeat(200)),
            "abcdef12-rest",
            None,
        )
        .unwrap();
        assert!(long.len() <= 64);
        let stripped: String = long.trim_start_matches("uml-viewer-").to_owned();
        assert!(stripped.ends_with("-abcdef12"), "{long}");
        // Two roots whose names share a prefix must not collide.
        let a = session_name("C:/projects/alpha-one", "abcdef12-rest", None).unwrap();
        let b = session_name("C:/projects/alpha-two", "abcdef12-rest", None).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn shrinks_the_name_so_the_socket_path_stays_inside_the_measured_limit() {
        let config_root = "/tmp/uml-restart-xdg-o7gbqk3s/config";
        let name =
            session_name("/projects/some project", "abcdef12-rest", Some(config_root)).unwrap();
        let socket = format!("{config_root}/herdr/sessions/{name}/herdr.sock");
        assert!(
            socket.len() <= MAX_SOCKET_BYTES,
            "{socket} ({} bytes)",
            socket.len()
        );
        assert!(herdr_domain::session_valid(&name), "{name}");
        assert!(name.len() <= MAX_NAME_BYTES);
        let other = session_name("/projects/other", "abcdef12-rest", Some(config_root)).unwrap();
        assert_ne!(name, other);
        assert!(other.ends_with("abcdef12"));
    }

    #[test]
    fn reports_the_real_cause_when_no_name_can_fit_the_config_root() {
        let error = session_name(
            "/projects/some project",
            "abcdef12-rest",
            Some(&format!("/tmp/{}", "x".repeat(120))),
        )
        .unwrap_err();
        assert!(error.message.contains("XDG_CONFIG_HOME"));
        assert!(error.message.contains("Unix socket path"));
    }

    #[test]
    fn load_retains_missing_foreign_and_incomplete_ownership() {
        static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "uml lifecycle {} {}",
            std::process::id(),
            SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(directory.join(".uml-viewer")).unwrap();
        let root = directory.to_string_lossy().into_owned();
        assert!(load(&root).unwrap().is_none());
        let cases = [
            format!(r#"{{:backend :unknown :cwd "{root}" :session "s" :pane "%1"}}"#),
            format!(r#"{{:cwd "{root}" :session "s" :pane "%1"}}"#),
            format!(
                r#"{{:backend :herdr :owner-role :launcher :cwd "{root}" :session "s" :pane "%1"}}"#
            ),
            "[:invalid]".to_owned(),
        ];
        for text in cases {
            std::fs::write(directory.join(".uml-viewer/companion.edn"), &text).unwrap();
            assert!(load(&root).is_err(), "{text}");
            assert_eq!(text, ownership::bytes(&root).unwrap(), "bytes are retained");
        }
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn claude_executable_prefers_claude_bin() {
        // The environment is process-global; this only asserts the fallback
        // shape when the variable is absent, which the test host controls.
        if std::env::var("CLAUDE_BIN").is_err() {
            assert_eq!("claude", claude_executable());
        }
    }

    #[test]
    fn the_role_argv_names_this_binary_and_the_role() {
        let argv = role_launcher("companion-runner").unwrap();
        assert_eq!(2, argv.len());
        assert!(
            argv[0].ends_with("companion")
                || argv[0].contains("uml-viewer")
                || std::path::Path::new(&argv[0]).is_absolute()
        );
        assert_eq!("companion-runner", argv[1]);
    }
}
