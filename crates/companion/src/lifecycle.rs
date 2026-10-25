//! Companion lifecycle orchestration (port of
//! `love/uml/adapters/companion_lifecycle.lua`). Prompt text, session naming
//! and ownership checks are pure; all process work stays in adapters.

use crate::domain;
use crate::herdr::{Config, ControllerSpec, Herdr, ProgressFn, RunnerLauncher, Selected};
use crate::herdr_domain;
use crate::host::Host;
use crate::ownership;
use crate::posix;
use crate::record;
use crate::recovery;
use crate::vocab::{Failure, Result, RunOut};
use edn::Value;

/// Herdr derives its API socket from XDG_CONFIG_HOME + session name. Measured
/// against Herdr 0.9.5: the server starts at 101 bytes but workspace creation
/// fails from 102 bytes, and that failure surfaces as an opaque closed-channel
/// error. The generated name therefore shrinks to whatever the config root
/// leaves, instead of assuming a fixed project-name budget.
pub const MAX_SOCKET_BYTES: usize = 101;
pub const MAX_NAME_BYTES: usize = 64;

pub fn config_root(host: &Host) -> String {
    config_root_in(&*host.variable)
}

fn config_root_in(var: &dyn Fn(&str) -> Option<String>) -> String {
    let root = var("XDG_CONFIG_HOME")
        .filter(|root| !root.is_empty())
        .unwrap_or_else(|| format!("{}/.config", var("HOME").unwrap_or_default()));
    root.trim_end_matches('/').to_owned()
}

/// `uml-viewer-<project>-<hash8>-<uuid8>`: the project name is the only part
/// that may shrink, so the socket path stays inside the measured limit.
pub fn session_name(root: &str, uuid: &str, config_root: &str) -> Result<String> {
    let base: String = root
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
    let socket_prefix = format!("{config_root}/herdr/sessions/");
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

/// Load and verify this project's complete Herdr ownership. Missing, legacy,
/// foreign or incomplete records are refused, never adopted.
pub fn load(root: &str, host: &Host) -> Result<Option<Value>> {
    let Some(value) = ownership::read_in(host, root)? else {
        return Ok(None);
    };
    if record::is_empty_map(&value) {
        return Ok(None);
    }
    domain::recorded_backend(Some(&value))?;
    herdr_domain::complete(&value)?;
    let cwd = record::get_name(&value, "cwd").unwrap_or_default();
    if (host.canonical)(&cwd) != (host.canonical)(root) {
        return Err(Failure::new(
            "Companion ownership is missing, ambiguous or belongs to another project; retained",
        ));
    }
    Ok(Some(value))
}

pub fn required(root: &str, host: &Host) -> Result<Value> {
    load(root, host)?.ok_or_else(|| {
        Failure::new("Companion ownership is missing, ambiguous or unreadable; retained")
    })
}

/// The companion executable: `CLAUDE_BIN` when set, otherwise the bare name.
pub fn claude_executable(host: &Host) -> String {
    (host.variable)("CLAUDE_BIN")
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "claude".to_owned())
}

pub(crate) fn current_exe() -> Result<String> {
    std::env::current_exe()
        .map(|path| path.to_string_lossy().into_owned())
        .map_err(|error| Failure::new(format!("cannot resolve the uml-viewer binary: {error}")))
}

/// The role argv for this binary: `[exe, role, manifest]`.
fn role_launcher(role: &str) -> Result<Vec<String>> {
    Ok(vec![current_exe()?, role.to_owned()])
}

/// This binary's runner role, as the pane launches it.
pub(crate) fn runner_launcher() -> Result<RunnerLauncher> {
    Ok(RunnerLauncher {
        argv: role_launcher("companion-runner")?,
    })
}

/// Read-only backend selection used before any resource is created.
pub fn select(root: &str, args: &[String]) -> Result<Selected> {
    crate::preflight::select(root, args, false)
}

/// The lifecycle backend configuration: the selected Herdr, the binary's own
/// runner and controller roles, and the worker's progress callback.
fn config(selected: Selected, progress: Option<ProgressFn>) -> Result<Config> {
    Ok(Config::lifecycle(
        selected,
        progress,
        runner_launcher()?,
        ControllerSpec {
            executable: current_exe()?,
            argv: vec!["companion-controller".to_owned()],
        },
    ))
}

pub fn create(root: &str, host: &Host, options_in: CreateOptions) -> Result<(Value, Herdr)> {
    if load(root, host)?.is_some() {
        return Err(Failure::new(
            "Companion ownership already recorded; use --restart or resolve it before fresh start",
        ));
    }
    let selected = select(root, &["--backend".to_owned(), "herdr".to_owned()])?;
    let claude = options_in
        .claude
        .clone()
        .unwrap_or_else(|| claude_executable(host));
    let uuid = options_in.uuid.clone().unwrap_or_else(|| (host.token)());
    let session = session_name(root, &uuid, &config_root(host))?;
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
    let canonical = (host.canonical)(root);
    let provisional = edn::record::record(vec![
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
    ownership::write(host, root, &provisional)?;
    // The backend builds every later record on this identity, so the committed
    // ownership keeps the --resume recovery command and the Claude session id.
    let mut backend = Herdr::new(
        Some(provisional),
        host.clone(),
        config(selected, options_in.progress)?,
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

pub fn restart(root: &str, host: &Host) -> Result<(Value, Herdr)> {
    let plain = load(root, host)?
        .ok_or_else(|| Failure::new("Companion ownership is missing; cannot restart"))?;
    let selected = select(root, &["--backend".to_owned(), "herdr".to_owned()])?;
    let backend = Herdr::new(Some(plain.clone()), host.clone(), config(selected, None)?);
    let session = record::get_name(&plain, "session").unwrap_or_default();
    let pane = record::get_name(&plain, "pane").unwrap_or_default();
    backend.probe(&session)?;
    backend.bind(&session, &pane)?;
    recovery::verify(&plain, host)?;
    Ok((plain, backend))
}

/// Cancel recovery first, then release only what this project verifiably
/// owns; ownership is cleared only after confirmed cleanup.
pub fn shutdown(root: &str, host: &Host) -> Result<bool> {
    let Some(plain) = load(root, host)? else {
        return Ok(false);
    };
    let selected = select(root, &["--backend".to_owned(), "herdr".to_owned()])?;
    let backend = Herdr::new(Some(plain.clone()), host.clone(), config(selected, None)?);
    let session = record::get_name(&plain, "session").unwrap_or_default();
    backend.cleanup(&session)?;
    ownership::write(host, root, &Value::Map(vec![]))?;
    Ok(true)
}

pub struct Opened {
    pub terminal: String,
    pub attach: String,
}

fn on_path(name: &str) -> Option<String> {
    std::env::var("PATH").ok().and_then(|path| {
        path.split(':')
            .map(|directory| format!("{directory}/{name}"))
            .find(|candidate| std::fs::File::open(candidate).is_ok())
    })
}

const HIDDEN_SIDEBAR: [(&str, &str); 2] = [
    ("sidebar_start_collapsed", "true"),
    ("sidebar_collapsed_mode", "\"hidden\""),
];

/// The person's Herdr config: `HERDR_CONFIG_PATH`, else under the config root.
fn user_config_path(var: &dyn Fn(&str) -> Option<String>) -> String {
    var("HERDR_CONFIG_PATH")
        .filter(|path| !path.is_empty())
        .unwrap_or_else(|| format!("{}/herdr/config.toml", config_root_in(var)))
}

fn table_name(header: &str) -> String {
    let inner = header
        .trim_start_matches('[')
        .split(']')
        .next()
        .unwrap_or("");
    inner
        .split('.')
        .map(str::trim)
        .collect::<Vec<_>>()
        .join(".")
}

/// `base` with the sidebar hidden: any existing sidebar collapse keys give
/// way to a hidden pair at the top of `[ui]`, which is appended when absent.
/// Line-based, so a config it cannot follow is caught by `config check`.
fn hidden_sidebar(base: &str) -> String {
    let settings = HIDDEN_SIDEBAR.map(|(key, value)| format!("{key} = {value}"));
    let mut out: Vec<String> = Vec::new();
    let mut table = String::new();
    let mut placed = false;
    for line in base.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            table = table_name(trimmed);
            out.push(line.to_owned());
            if table == "ui" && !placed {
                out.extend(settings.iter().cloned());
                placed = true;
            }
            continue;
        }
        if let Some((key, _)) = trimmed.split_once('=') {
            let key = key.split('.').map(str::trim).collect::<Vec<_>>().join(".");
            let full = if table.is_empty() {
                key
            } else {
                format!("{table}.{key}")
            };
            if HIDDEN_SIDEBAR
                .iter()
                .any(|(name, _)| full == format!("ui.{name}"))
            {
                continue;
            }
        }
        out.push(line.to_owned());
    }
    if !placed {
        if !out.is_empty() {
            out.push(String::new());
        }
        out.push("[ui]".to_owned());
        out.extend(settings);
    }
    out.join("\n") + "\n"
}

/// The attach client, not the server, reads the sidebar settings (measured on
/// Herdr 0.9.5), so the companion's client gets the person's config with the
/// sidebar hidden. A config Herdr rejects falls back to theirs unchanged.
fn attach_config(root: &str, herdr: &str) -> Option<String> {
    let var = |name: &str| std::env::var(name).ok();
    let base = std::fs::read_to_string(user_config_path(&var)).unwrap_or_default();
    let path = format!(
        "{}/.uml-viewer/herdr-attach.toml",
        root.trim_end_matches('/')
    );
    std::fs::write(&path, hidden_sidebar(&base)).ok()?;
    let argv: Vec<std::ffi::OsString> = [
        "/usr/bin/env",
        &format!("HERDR_CONFIG_PATH={path}"),
        herdr,
        "config",
        "check",
    ]
    .iter()
    .map(std::ffi::OsString::from)
    .collect();
    let check = posix::run(&argv, Some(10000), &[]);
    (check.exit == Some(0) && !check.timeout).then_some(path)
}

/// The `/usr/bin/env` prefix that carries the config root and the attach
/// config into the new tab; empty when neither applies. Windows Terminal
/// splits its command line at `;` even inside quotes, so an attach config
/// under such a project path is left out and the tab reads the person's own.
fn attach_env(config_root: Option<String>, attach_config: Option<&str>) -> Vec<String> {
    let mut assignments: Vec<String> = config_root
        .filter(|root| !root.is_empty())
        .map(|root| format!("XDG_CONFIG_HOME={root}"))
        .into_iter()
        .collect();
    assignments.extend(
        attach_config
            .filter(|path| !path.contains(';'))
            .map(|path| format!("HERDR_CONFIG_PATH={path}")),
    );
    if assignments.is_empty() {
        return assignments;
    }
    assignments.insert(0, "/usr/bin/env".to_owned());
    assignments
}

/// WSL reaches the host's Windows Terminal through interop; the new tab enters
/// the same distribution and attaches with the selected Herdr's absolute path.
/// Elsewhere, or when the terminal does not start, the caller reports the
/// command a person can run (ADR 0007).
pub fn open_window(root: &str, session: &str, herdr: &str) -> Opened {
    let config = attach_config(root, herdr);
    let attach = format!(
        "{}{} session attach {}",
        config
            .as_ref()
            .map(|path| format!("HERDR_CONFIG_PATH={} ", crate::text::shell_word(path)))
            .unwrap_or_default(),
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
    argv.extend(attach_env(
        std::env::var("XDG_CONFIG_HOME").ok(),
        config.as_deref(),
    ));
    for word in [herdr, "session", "attach", session] {
        argv.push(word.to_owned());
    }
    let os_argv: Vec<std::ffi::OsString> = argv.iter().map(std::ffi::OsString::from).collect();
    let result: RunOut = posix::run(&os_argv, Some(10000), &[]);
    let started = result.exit == Some(0) && !result.timeout;
    Opened {
        terminal: if started { "windows-terminal" } else { "none" }.to_owned(),
        attach,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    #[test]
    fn the_attach_config_hides_the_sidebar_and_keeps_every_other_setting() {
        let base = "onboarding = false\n[ui.toast]\ndelivery = \"herdr\"\n\n[ui]\nsidebar_start_collapsed = false\nstatus_indicators = \"symbols\"\nsidebar_collapsed_mode = \"compact\"\n[theme]\nname = \"x\"\n";
        assert_eq!(
            "onboarding = false\n[ui.toast]\ndelivery = \"herdr\"\n\n[ui]\nsidebar_start_collapsed = true\nsidebar_collapsed_mode = \"hidden\"\nstatus_indicators = \"symbols\"\n[theme]\nname = \"x\"\n",
            hidden_sidebar(base)
        );
        assert_eq!(
            "ui.sidebar_width = 30\n[theme]\nname = \"x\"\n\n[ui]\nsidebar_start_collapsed = true\nsidebar_collapsed_mode = \"hidden\"\n",
            hidden_sidebar(
                "ui.sidebar_width = 30\nui.sidebar_start_collapsed=false\n[theme]\nname = \"x\"\n"
            )
        );
        assert_eq!(
            "[ui]\nsidebar_start_collapsed = true\nsidebar_collapsed_mode = \"hidden\"\n",
            hidden_sidebar("")
        );
    }

    #[test]
    fn the_new_tab_receives_the_config_root_and_the_attach_config() {
        assert_eq!(Vec::<String>::new(), attach_env(None, None));
        assert_eq!(Vec::<String>::new(), attach_env(Some(String::new()), None));
        assert_eq!(
            vec![
                "/usr/bin/env".to_owned(),
                "XDG_CONFIG_HOME=/x".to_owned(),
                "HERDR_CONFIG_PATH=/p/a.toml".to_owned()
            ],
            attach_env(Some("/x".to_owned()), Some("/p/a.toml"))
        );
        assert_eq!(
            vec![
                "/usr/bin/env".to_owned(),
                "HERDR_CONFIG_PATH=/p/a.toml".to_owned()
            ],
            attach_env(None, Some("/p/a.toml"))
        );
        assert_eq!(
            vec!["/usr/bin/env".to_owned(), "XDG_CONFIG_HOME=/x".to_owned()],
            attach_env(Some("/x".to_owned()), Some("/p;q/a.toml"))
        );
        assert_eq!(Vec::<String>::new(), attach_env(None, Some("/p;q/a.toml")));
    }

    #[test]
    fn the_person_s_herdr_config_follows_the_override_then_the_config_root() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| {
                pairs
                    .iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, value)| (*value).to_owned())
            }
        };
        let all = env(&[
            ("HERDR_CONFIG_PATH", "/c/herdr.toml"),
            ("XDG_CONFIG_HOME", "/x/"),
            ("HOME", "/h"),
        ]);
        assert_eq!("/c/herdr.toml", user_config_path(&all));
        let xdg = env(&[
            ("HERDR_CONFIG_PATH", ""),
            ("XDG_CONFIG_HOME", "/x/"),
            ("HOME", "/h"),
        ]);
        assert_eq!("/x/herdr/config.toml", user_config_path(&xdg));
        let home = env(&[("HOME", "/h")]);
        assert_eq!("/h/.config/herdr/config.toml", user_config_path(&home));
    }

    /// A typical `$HOME/.config` root; no test reads the process environment.
    const HOME_CONFIG: &str = "/home/person/.config";

    #[test]
    fn names_a_fresh_session_uniquely_per_launch_within_the_grammar() {
        let name = session_name("/projects/some project", "abcdef12-rest", HOME_CONFIG).unwrap();
        assert!(name.len() <= 64);
        assert!(herdr_domain::session_valid(&name), "{name}");
        let long = session_name(
            &format!("/projects/{}", "x".repeat(200)),
            "abcdef12-rest",
            HOME_CONFIG,
        )
        .unwrap();
        assert!(long.len() <= 64);
        let stripped: String = long.trim_start_matches("uml-viewer-").to_owned();
        assert!(stripped.ends_with("-abcdef12"), "{long}");
        // Two roots whose names share a prefix must not collide.
        let a = session_name("/projects/alpha-one", "abcdef12-rest", HOME_CONFIG).unwrap();
        let b = session_name("/projects/alpha-two", "abcdef12-rest", HOME_CONFIG).unwrap();
        assert_ne!(a, b);
    }

    #[test]
    fn shrinks_the_name_so_the_socket_path_stays_inside_the_measured_limit() {
        let config_root = "/tmp/uml-restart-xdg-o7gbqk3s/config";
        let name = session_name("/projects/some project", "abcdef12-rest", config_root).unwrap();
        let socket = format!("{config_root}/herdr/sessions/{name}/herdr.sock");
        assert!(
            socket.len() <= MAX_SOCKET_BYTES,
            "{socket} ({} bytes)",
            socket.len()
        );
        assert!(herdr_domain::session_valid(&name), "{name}");
        assert!(name.len() <= MAX_NAME_BYTES);
        let other = session_name("/projects/other", "abcdef12-rest", config_root).unwrap();
        assert_ne!(name, other);
        assert!(other.ends_with("abcdef12"));
    }

    #[test]
    fn reports_the_real_cause_when_no_name_can_fit_the_config_root() {
        let error = session_name(
            "/projects/some project",
            "abcdef12-rest",
            &format!("/tmp/{}", "x".repeat(120)),
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
        let host = Host::posix();
        assert!(load(&root, &host).unwrap().is_none());
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
            assert!(load(&root, &host).is_err(), "{text}");
            assert_eq!(text, ownership::bytes(&root).unwrap(), "bytes are retained");
        }
        let _ = std::fs::remove_dir_all(&directory);
    }

    #[test]
    fn claude_executable_prefers_claude_bin() {
        let with = |value: Option<&'static str>| Host {
            variable: Rc::new(move |name| {
                assert_eq!("CLAUDE_BIN", name);
                value.map(str::to_owned)
            }),
            ..Host::fake()
        };
        assert_eq!("/opt/claude", claude_executable(&with(Some("/opt/claude"))));
        assert_eq!("claude", claude_executable(&with(Some(""))));
        assert_eq!("claude", claude_executable(&with(None)));
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
