//! Ported from `love/spec/herdr_spec.lua`: the fake CLI answers the Herdr
//! protocol exactly and every call selects the recorded session.

use super::*;
use crate::json::Json;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

fn owned() -> Value {
    edn::parse(
        r#"{:backend :herdr :owner-role :companion :session "uml-viewer-A"
            :socket "/x/herdr.sock" :workspace "w1" :pane "w1:p1" :terminal-id "term_A"
            :shell-pid 42 :owner-start "shell-start" :server-pid "41"
            :server-start "server-start" :cwd "/nonexistent-uml-viewer-fixture/project"
            :command ["claude"]}"#,
    )
    .unwrap()
}

#[derive(Default)]
struct FakeState {
    stopped: bool,
    closed: bool,
    deleted: bool,
    ignore_close: bool,
    empty_sessions: bool,
    no_agent: bool,
    terminal: Option<String>,
    cwd: Option<String>,
    server_start: Option<String>,
    shell_start: Option<String>,
    fail: Option<(String, String)>,
    prompted: bool,
    run_text: Option<String>,
    foreground: Option<Json>,
    foreground_absent: bool,
    reset_fail: bool,
}

type Calls = Rc<RefCell<Vec<Vec<String>>>>;
type Shared = Rc<RefCell<FakeState>>;

fn response(value: Json) -> RunOut {
    RunOut {
        exit: Some(0),
        out: crate::json::encode(&value),
        err: String::new(),
        timeout: false,
    }
}

fn object(entries: Vec<(&str, Json)>) -> Json {
    Json::Object(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect::<BTreeMap<_, _>>(),
    )
}

fn fake_cli(calls: Calls, state: Shared) -> impl Fn(&[String], Option<i64>) -> RunOut {
    move |argv, _| {
        calls.borrow_mut().push(argv.to_vec());
        let socket = "/x/herdr.sock".to_owned();
        assert_eq!("herdr", argv[0]);
        assert_eq!("--session", argv[1]);
        assert_eq!("uml-viewer-A", argv[2]);
        let op = (
            argv[3].as_str(),
            argv.get(4).map(String::as_str).unwrap_or(""),
        );
        {
            let state = state.borrow();
            if let Some(fail) = &state.fail
                && fail.0 == op.0
                && fail.1 == op.1
            {
                return RunOut {
                    exit: Some(1),
                    out: String::new(),
                    err: "injected failure".to_owned(),
                    timeout: false,
                };
            }
        }
        match op {
            ("status", "server") => {
                let stopped = state.borrow().stopped;
                response(object(vec![
                    ("session", Json::String("uml-viewer-A".to_owned())),
                    ("socket", Json::String(socket)),
                    ("running", Json::Bool(!stopped)),
                    ("version", Json::String("0.9.3".to_owned())),
                    ("protocol", Json::Number(22.0)),
                    ("compatible", Json::Bool(true)),
                    ("endpoint_compatible", Json::Bool(true)),
                ]))
            }
            ("api", "snapshot") => {
                let state = state.borrow();
                let panes = Json::Array(vec![object(vec![
                    ("pane_id", Json::String("w1:p1".to_owned())),
                    ("workspace_id", Json::String("w1".to_owned())),
                    (
                        "terminal_id",
                        Json::String(
                            state
                                .terminal
                                .clone()
                                .unwrap_or_else(|| "term_A".to_owned()),
                        ),
                    ),
                    (
                        "cwd",
                        Json::String(state.cwd.clone().unwrap_or_else(|| {
                            "/nonexistent-uml-viewer-fixture/project".to_owned()
                        })),
                    ),
                ])]);
                let workspaces = if state.closed {
                    Json::Array(vec![])
                } else {
                    Json::Array(vec![object(vec![(
                        "workspace_id",
                        Json::String("w1".to_owned()),
                    )])])
                };
                let agents = if state.no_agent {
                    Json::Array(vec![])
                } else {
                    Json::Array(vec![object(vec![
                        ("pane_id", Json::String("w1:p1".to_owned())),
                        ("terminal_id", Json::String("term_A".to_owned())),
                        ("agent", Json::String("claude".to_owned())),
                    ])])
                };
                response(object(vec![(
                    "result",
                    object(vec![(
                        "snapshot",
                        object(vec![
                            ("version", Json::String("0.9.3".to_owned())),
                            ("protocol", Json::Number(22.0)),
                            ("workspaces", workspaces),
                            ("panes", panes),
                            ("agents", agents),
                        ]),
                    )]),
                )]))
            }
            ("pane", "process-info") => {
                let state = state.borrow();
                let foreground = if state.foreground_absent {
                    Json::Null
                } else if let Some(foreground) = &state.foreground {
                    foreground.clone()
                } else {
                    Json::Array(vec![object(vec![
                        ("pid", Json::Number(42.0)),
                        ("name", Json::String("bash".to_owned())),
                    ])])
                };
                response(object(vec![(
                    "result",
                    object(vec![(
                        "process_info",
                        object(vec![
                            ("pane_id", Json::String("w1:p1".to_owned())),
                            ("shell_pid", Json::Number(42.0)),
                            ("foreground_processes", foreground),
                        ]),
                    )]),
                )]))
            }
            ("workspace", "create") => response(object(vec![(
                "result",
                object(vec![
                    (
                        "workspace",
                        object(vec![("workspace_id", Json::String("w1".to_owned()))]),
                    ),
                    (
                        "root_pane",
                        object(vec![
                            ("pane_id", Json::String("w1:p1".to_owned())),
                            ("terminal_id", Json::String("term_A".to_owned())),
                        ]),
                    ),
                ]),
            )])),
            ("workspace", "close") => {
                let mut state = state.borrow_mut();
                if !state.ignore_close {
                    state.closed = true;
                }
                response(object(vec![(
                    "result",
                    object(vec![("type", Json::String("ok".to_owned()))]),
                )]))
            }
            ("session", "stop") => {
                state.borrow_mut().stopped = true;
                response(object(vec![
                    ("stopped", Json::Bool(true)),
                    (
                        "session",
                        object(vec![
                            ("name", Json::String("uml-viewer-A".to_owned())),
                            ("socket_path", Json::String(socket)),
                        ]),
                    ),
                ]))
            }
            ("session", "delete") => {
                state.borrow_mut().deleted = true;
                response(object(vec![
                    ("deleted", Json::Bool(true)),
                    (
                        "session",
                        object(vec![
                            ("name", Json::String("uml-viewer-A".to_owned())),
                            ("socket_path", Json::String(socket)),
                        ]),
                    ),
                ]))
            }
            ("session", "list") => {
                let state = state.borrow();
                let sessions = if state.empty_sessions {
                    Json::Array(vec![])
                } else if state.deleted {
                    Json::Array(vec![object(vec![(
                        "name",
                        Json::String("uml-viewer-B".to_owned()),
                    )])])
                } else {
                    Json::Array(vec![
                        object(vec![("name", Json::String("uml-viewer-A".to_owned()))]),
                        object(vec![("name", Json::String("uml-viewer-B".to_owned()))]),
                    ])
                };
                response(object(vec![("sessions", sessions)]))
            }
            ("agent", "prompt") => {
                state.borrow_mut().prompted = true;
                response(object(vec![(
                    "result",
                    object(vec![("type", Json::String("ok".to_owned()))]),
                )]))
            }
            ("pane", "send-keys") => response(object(vec![(
                "result",
                object(vec![("type", Json::String("ok".to_owned()))]),
            )])),
            ("pane", "run") => {
                state.borrow_mut().run_text = argv.get(6).cloned();
                response(object(vec![(
                    "result",
                    object(vec![("type", Json::String("ok".to_owned()))]),
                )]))
            }
            // A fresh pane shows its prompt before the first `pane run` lands
            // (measured on 0.9.5); the adapter waits for this text, which the
            // CLI prints raw rather than as JSON.
            ("pane", "read") => RunOut {
                exit: Some(0),
                out: "❯ ".to_owned(),
                err: String::new(),
                timeout: false,
            },
            other => panic!("Unexpected CLI call: {other:?}"),
        }
    }
}

fn fake_owner(state: Shared) -> impl Fn(i64) -> Option<Owner> {
    move |pid| {
        let state = state.borrow();
        match pid {
            41 if !state.stopped => Some(Owner {
                pid: 41,
                start: state
                    .server_start
                    .clone()
                    .unwrap_or_else(|| "server-start".to_owned()),
                name: "herdr".to_owned(),
            }),
            42 => Some(Owner {
                pid: 42,
                start: state
                    .shell_start
                    .clone()
                    .unwrap_or_else(|| "shell-start".to_owned()),
                name: "bash".to_owned(),
            }),
            _ => None,
        }
    }
}

fn backend(
    run: impl Fn(&[String], Option<i64>) -> RunOut + 'static,
    owner: impl Fn(i64) -> Option<Owner> + 'static,
) -> Herdr {
    Herdr::new(
        Some(owned()),
        Options {
            run: Rc::new(run),
            owner: Rc::new(owner),
            clock: Rc::new(|| 0),
            sleep: Rc::new(|_| {}),
            canonical: Rc::new(|path| path.to_owned()),
            selected: Some(Selected {
                path: "herdr".to_owned(),
                release: "0.9.3".to_owned(),
            }),
            write_companion: Rc::new(|_, _| Ok(())),
            read_companion: Rc::new(|_| Ok(None)),
            ..Options::real()
        },
    )
}

fn ops(calls: &Calls) -> Vec<String> {
    calls
        .borrow()
        .iter()
        .filter(|argv| argv[0] == "herdr")
        .map(|argv| format!("{} {}", argv[3], argv.get(4).cloned().unwrap_or_default()))
        .collect()
}

#[test]
fn binds_the_full_identity_via_explicit_session_selectors() {
    let calls: Calls = Rc::new(RefCell::new(Vec::new()));
    let state: Shared = Rc::new(RefCell::new(FakeState::default()));
    let backend = backend(fake_cli(calls.clone(), state.clone()), fake_owner(state));
    assert_eq!("w1:p1", backend.bind("uml-viewer-A", "w1:p1").unwrap());
    assert!(backend.probe("uml-viewer-A").unwrap());
}

#[test]
fn rejects_a_and_b_selectors_even_though_both_sessions_use_the_same_pane_id() {
    let calls: Calls = Rc::new(RefCell::new(Vec::new()));
    let state: Shared = Rc::new(RefCell::new(FakeState::default()));
    let backend = backend(fake_cli(calls.clone(), state.clone()), fake_owner(state));
    assert!(backend.bind("uml-viewer-B", "w1:p1").is_err());
    assert!(calls.borrow().is_empty());
}

#[test]
fn refuses_cleanup_when_terminal_shell_or_server_creation_identity_changes() {
    for change in ["terminal", "shell_start", "server_start"] {
        let calls: Calls = Rc::new(RefCell::new(Vec::new()));
        let state: Shared = Rc::new(RefCell::new(FakeState::default()));
        {
            let mut state = state.borrow_mut();
            match change {
                "terminal" => state.terminal = Some("term_B".to_owned()),
                "shell_start" => state.shell_start = Some("reused".to_owned()),
                _ => state.server_start = Some("reused".to_owned()),
            }
        }
        let backend = backend(fake_cli(calls.clone(), state.clone()), fake_owner(state));
        assert!(backend.cleanup("uml-viewer-A").is_err(), "{change}");
        let seen = ops(&calls).join(",");
        assert!(!seen.contains("workspace close"), "{change}: {seen}");
        assert!(!seen.contains("session stop"), "{change}: {seen}");
    }
}

#[test]
fn closes_and_confirms_only_a_while_leaving_b_in_the_inventory() {
    let calls: Calls = Rc::new(RefCell::new(Vec::new()));
    let state: Shared = Rc::new(RefCell::new(FakeState::default()));
    let backend = backend(
        fake_cli(calls.clone(), state.clone()),
        fake_owner(state.clone()),
    );
    backend.cleanup("uml-viewer-A").unwrap();
    assert!(state.borrow().deleted);
    for argv in calls.borrow().iter() {
        if argv[0] == "herdr" {
            assert_eq!("uml-viewer-A", argv[2]);
        }
    }
}

#[test]
fn retains_ambiguous_cleanup_after_failure_or_a_false_successful_close() {
    for change in ["fail close", "ignore close", "fail stop"] {
        let calls: Calls = Rc::new(RefCell::new(Vec::new()));
        let state: Shared = Rc::new(RefCell::new(FakeState::default()));
        {
            let mut state = state.borrow_mut();
            match change {
                "fail close" => state.fail = Some(("workspace".to_owned(), "close".to_owned())),
                "ignore close" => state.ignore_close = true,
                _ => state.fail = Some(("session".to_owned(), "stop".to_owned())),
            }
        }
        let backend = backend(
            fake_cli(calls.clone(), state.clone()),
            fake_owner(state.clone()),
        );
        assert!(backend.cleanup("uml-viewer-A").is_err(), "{change}");
        assert!(!state.borrow().deleted, "{change}");
    }
}

#[test]
fn never_treats_malformed_or_timed_out_probes_as_absence() {
    for result in [
        RunOut {
            exit: Some(0),
            out: "not JSON".to_owned(),
            err: String::new(),
            timeout: false,
        },
        RunOut {
            exit: None,
            out: String::new(),
            err: "timeout".to_owned(),
            timeout: true,
        },
    ] {
        let state: Shared = Rc::new(RefCell::new(FakeState::default()));
        let answer = result.clone();
        let backend = backend(move |_, _| answer.clone(), fake_owner(state));
        assert!(backend.probe("uml-viewer-A").is_err());
    }
}

#[test]
fn wake_requires_a_fresh_acknowledgement_and_a_real_claude_occupant() {
    for no_agent in [true, false] {
        let calls: Calls = Rc::new(RefCell::new(Vec::new()));
        let state: Shared = Rc::new(RefCell::new(FakeState {
            no_agent,
            ..FakeState::default()
        }));
        let backend = backend(fake_cli(calls, state.clone()), fake_owner(state.clone()));
        // Never acknowledged: delivery fails; the prompt is sent only when a
        // Claude agent occupies the pane.
        let result = backend.wake(
            "w1:p1",
            &["-l".to_owned(), "mail token-1".to_owned()],
            "token-1",
            &|_| false,
            Some(0),
            None,
        );
        assert!(result.is_err());
        assert_eq!(!no_agent, state.borrow().prompted);
    }
}

#[test]
fn wake_returns_success_only_after_this_prompt_was_consumed() {
    let calls: Calls = Rc::new(RefCell::new(Vec::new()));
    let state: Shared = Rc::new(RefCell::new(FakeState::default()));
    let consumed = state.clone();
    let backend = backend(fake_cli(calls, state.clone()), fake_owner(state));
    backend
        .wake(
            "w1:p1",
            &["-l".to_owned(), "mail token-2".to_owned()],
            "token-2",
            &|token| token == "token-2" && consumed.borrow().prompted,
            Some(5000),
            Some(0),
        )
        .unwrap();
}

#[test]
fn wake_rejects_preexisting_acknowledgements_and_missing_tokens() {
    let calls: Calls = Rc::new(RefCell::new(Vec::new()));
    let state: Shared = Rc::new(RefCell::new(FakeState::default()));
    let backend = backend(fake_cli(calls, state.clone()), fake_owner(state.clone()));
    // No token embedded in the literal text.
    assert!(
        backend
            .wake(
                "w1:p1",
                &["-l".to_owned(), "mail".to_owned()],
                "",
                &|_| false,
                None,
                None
            )
            .is_err()
    );
    // Acknowledged before this delivery.
    assert!(
        backend
            .wake(
                "w1:p1",
                &["-l".to_owned(), "mail token".to_owned()],
                "token",
                &|_| true,
                None,
                None,
            )
            .is_err()
    );
    assert!(!state.borrow().prompted);
}

#[test]
fn stale_or_malformed_runner_state_is_ignored() {
    let directory = std::env::temp_dir().join(format!(
        "uml herdr test {} {}",
        std::process::id(),
        posix::monotonic_ms()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("runner.edn");
    std::fs::write(&path, r#"{"token":"stale","phase":"started"}"#).unwrap();
    let calls: Calls = Rc::new(RefCell::new(Vec::new()));
    let state: Shared = Rc::new(RefCell::new(FakeState::default()));
    let backend = backend(fake_cli(calls, state.clone()), fake_owner(state));
    let record = record::map(vec![
        (
            "runner-state",
            Value::String(path.to_string_lossy().into_owned()),
        ),
        ("runner-token", Value::String("current".to_owned())),
    ]);
    assert!(backend.runner_state(&record).is_none());
    std::fs::write(&path, "broken JSON").unwrap();
    assert!(backend.runner_state(&record).is_none());
    std::fs::write(&path, r#"{"token":"current","phase":"exited","exit":37}"#).unwrap();
    let value = backend.runner_state(&record).unwrap();
    assert_eq!(Some(37), record::get_i64(&value, "exit"));
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn automatic_recovery_without_a_durable_supervisor_is_refused() {
    let calls: Calls = Rc::new(RefCell::new(Vec::new()));
    let state: Shared = Rc::new(RefCell::new(FakeState::default()));
    let backend = backend(fake_cli(calls, state.clone()), fake_owner(state));
    assert!(!backend.recovery("uml-viewer-A", None, false).unwrap());
    assert!(backend.recovery("uml-viewer-A", None, true).is_err());
}

#[test]
fn socket_paths_over_the_measured_limit_are_refused_before_any_command() {
    let calls: Calls = Rc::new(RefCell::new(Vec::new()));
    let recorded = calls.clone();
    let run = move |argv: &[String], _: Option<i64>| {
        recorded.borrow_mut().push(argv.to_vec());
        RunOut {
            exit: Some(1),
            out: String::new(),
            err: "CLI reached".to_owned(),
            timeout: false,
        }
    };
    let state: Shared = Rc::new(RefCell::new(FakeState::default()));
    let mut backend = backend(run, fake_owner(state));
    backend.identity = Some(record::map(vec![("provisional", Value::Bool(true))]));
    backend.options.max_socket_bytes = 1;
    let error = backend
        .create("uml-viewer-fixture", "/tmp", &["claude".to_owned()])
        .unwrap_err();
    assert!(
        error.message.contains("Unix socket limit"),
        "{}",
        error.message
    );
    assert!(calls.borrow().is_empty());
}

#[test]
fn pane_run_retries_only_canonical_mode_rejections_within_the_deadline() {
    // Other failures: a single attempt, no sleep.
    let attempts = Rc::new(RefCell::new(0));
    let sleeps = Rc::new(RefCell::new(0));
    let counted = attempts.clone();
    let slept = sleeps.clone();
    let mut first = backend(
        move |_, _| {
            *counted.borrow_mut() += 1;
            RunOut {
                exit: Some(1),
                out: String::new(),
                err: "pane_not_found".to_owned(),
                timeout: false,
            }
        },
        |_| None,
    );
    first.options.sleep = Rc::new(move |_| *slept.borrow_mut() += 1);
    assert!(first.pane_run("uml-viewer-A", "w1:p1", "text").is_err());
    assert_eq!(1, *attempts.borrow());
    assert_eq!(0, *sleeps.borrow());

    // Canonical mode: retried until the deadline is exhausted.
    let ticks = Rc::new(RefCell::new(vec![0i64, 5000, 20000]));
    let attempts = Rc::new(RefCell::new(0));
    let counted = attempts.clone();
    let clock_ticks = ticks.clone();
    let mut canonical_backend = backend(
        move |_, _| {
            *counted.borrow_mut() += 1;
            RunOut {
                exit: Some(1),
                out: String::new(),
                err: "pane_send_failed: input exceeds the canonical mode limit".to_owned(),
                timeout: false,
            }
        },
        |_| None,
    );
    canonical_backend.options.clock = Rc::new(move || {
        let mut ticks = clock_ticks.borrow_mut();
        if ticks.len() > 1 {
            ticks.remove(0)
        } else {
            ticks[0]
        }
    });
    assert!(
        canonical_backend
            .pane_run("uml-viewer-A", "w1:p1", "text")
            .is_err()
    );
    assert_eq!(2, *attempts.borrow());
}

#[test]
fn create_refuses_a_command_that_is_not_resolvable() {
    let calls: Calls = Rc::new(RefCell::new(Vec::new()));
    let state: Shared = Rc::new(RefCell::new(FakeState {
        empty_sessions: true,
        ..FakeState::default()
    }));
    let mut backend = backend(fake_cli(calls, state.clone()), fake_owner(state));
    backend.identity = Some(record::map(vec![("provisional", Value::Bool(true))]));
    let error = backend
        .create(
            "uml-viewer-A",
            "/tmp",
            &["definitely-not-on-path-106".to_owned()],
        )
        .unwrap_err();
    assert!(
        error.message.contains("not found on PATH"),
        "{}",
        error.message
    );
}

#[test]
fn a_command_without_argv_is_refused() {
    let calls: Calls = Rc::new(RefCell::new(Vec::new()));
    let state: Shared = Rc::new(RefCell::new(FakeState::default()));
    let mut backend = backend(fake_cli(calls, state.clone()), fake_owner(state));
    backend.identity = Some(record::map(vec![("provisional", Value::Bool(true))]));
    assert!(backend.create("uml-viewer-A", "/tmp", &[]).is_err());
}

fn create_backend(
    state: Shared,
    calls: Calls,
    checkpoints: Calls2,
    recovered: Rc<RefCell<Option<Value>>>,
) -> Herdr {
    let run_state = state.clone();
    let run_calls = calls.clone();
    Herdr::new(
        Some(record::map(vec![("provisional", Value::Bool(true))])),
        Options {
            run: Rc::new(move |argv, timeout| {
                fake_cli(run_calls.clone(), run_state.clone())(argv, timeout)
            }),
            owner: Rc::new(fake_owner(state.clone())),
            clock: Rc::new(|| 0),
            sleep: Rc::new(|_| {}),
            canonical: Rc::new(|path| path.to_owned()),
            selected: Some(Selected {
                path: "herdr".to_owned(),
                release: "0.9.3".to_owned(),
            }),
            write_companion: Rc::new(move |_, record| {
                checkpoints.borrow_mut().push(record.clone());
                Ok(())
            }),
            read_companion: Rc::new(|_| Ok(None)),
            spawn_server: Some(Rc::new(|_, _, _, _| {
                Ok(Some(Owner {
                    pid: 41,
                    start: "server-start".to_owned(),
                    name: "herdr".to_owned(),
                }))
            })),
            runner: Some(RunnerLauncher {
                argv: vec![
                    "love".to_owned(),
                    "/game".to_owned(),
                    "companion-runner".to_owned(),
                ],
            }),
            runner_state: Some(Rc::new(|record| {
                Some(record::map(vec![
                    (
                        "token",
                        record::get(record, "runner-token")
                            .cloned()
                            .unwrap_or(Value::Nil),
                    ),
                    ("phase", Value::keyword("started")),
                ]))
            })),
            start_recovery: Some(Rc::new(move |record, _| {
                *recovered.borrow_mut() = Some(record.clone());
                Ok(())
            })),
            ..Options::real()
        },
    )
}

type Calls2 = Rc<RefCell<Vec<Value>>>;

/// Full create flow with the fake CLI: provisional checkpoints land before
/// the runner is submitted, and the committed ownership keeps the original
/// command argv.
#[test]
fn create_persists_provisional_identity_before_run_and_returns_original_argv() {
    let bin = std::env::temp_dir().join(format!(
        "uml herdr create {} {}",
        std::process::id(),
        posix::monotonic_ms()
    ));
    std::fs::create_dir_all(&bin).unwrap();
    let exe = bin.join("claude 中文");
    std::fs::write(&exe, "").unwrap();
    let root = bin.join("project");
    std::fs::create_dir_all(&root).unwrap();
    let state: Shared = Rc::new(RefCell::new(FakeState {
        empty_sessions: true,
        cwd: Some(root.to_string_lossy().into_owned()),
        ..FakeState::default()
    }));
    let calls: Calls = Rc::new(RefCell::new(Vec::new()));
    let checkpoints: Calls2 = Rc::new(RefCell::new(Vec::new()));
    let recovered: Rc<RefCell<Option<Value>>> = Rc::new(RefCell::new(None));
    let mut backend = create_backend(state.clone(), calls, checkpoints.clone(), recovered.clone());
    let original = vec![
        exe.to_string_lossy().into_owned(),
        "space $ & ' 中文".to_owned(),
        String::new(),
    ];
    let identity = backend
        .create("uml-viewer-A", &root.to_string_lossy(), &original)
        .unwrap();
    assert_eq!(
        original,
        record::string_vec(record::get_vec(&identity, "command"))
    );
    assert_eq!(None, record::get(&identity, "provisional"));
    assert_eq!(
        Some("term_A".to_owned()),
        record::get_name(&identity, "terminal-id")
    );
    assert!(record::get_name(&identity, "runner-token").is_some());
    let before_run = checkpoints
        .borrow()
        .iter()
        .any(|record| record::get_bool(record, "provisional") == Some(true));
    assert!(before_run);
    for entry in checkpoints.borrow().iter() {
        if record::get(entry, "pane").is_some() && record::get(entry, "shell-pid").is_some() {
            assert_eq!(Some(42), record::get_i64(entry, "shell-pid"));
            assert_eq!(
                Some("shell-start".to_owned()),
                record::get_name(entry, "owner-start")
            );
        }
    }
    assert!(state.borrow().run_text.is_some());
    let recovered = recovered.borrow().clone().expect("recovery registered");
    assert_eq!(None, record::get(&recovered, "provisional"));
    let _ = std::fs::remove_dir_all(&bin);
}

#[test]
fn create_waits_for_a_server_bootstrap_beyond_the_ordinary_bound() {
    let root = std::env::temp_dir().join(format!(
        "uml herdr bootstrap {} {}",
        std::process::id(),
        posix::monotonic_ms()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let exe = root.join("claude");
    std::fs::write(&exe, "").unwrap();
    let state: Shared = Rc::new(RefCell::new(FakeState {
        empty_sessions: true,
        cwd: Some(root.to_string_lossy().into_owned()),
        ..FakeState::default()
    }));
    let calls: Calls = Rc::new(RefCell::new(Vec::new()));
    let run_state = state.clone();
    let run_calls = calls.clone();
    let slow = move |argv: &[String], timeout_ms: Option<i64>| {
        if argv.get(3).map(String::as_str) == Some("workspace")
            && argv.get(4).map(String::as_str) == Some("create")
            && timeout_ms.is_none_or(|timeout| timeout <= 12000)
        {
            return RunOut {
                exit: None,
                out: String::new(),
                err: "process timed out".to_owned(),
                timeout: true,
            };
        }
        fake_cli(run_calls.clone(), run_state.clone())(argv, timeout_ms)
    };
    let checkpoints: Calls2 = Rc::new(RefCell::new(Vec::new()));
    let recovered: Rc<RefCell<Option<Value>>> = Rc::new(RefCell::new(None));
    let mut backend = create_backend(state, calls, checkpoints, recovered);
    backend.options.run = Rc::new(slow);
    let identity = backend
        .create(
            "uml-viewer-A",
            &root.to_string_lossy(),
            &[exe.to_string_lossy().into_owned()],
        )
        .unwrap();
    assert_eq!(
        Some("w1:p1".to_owned()),
        record::get_name(&identity, "pane")
    );
    let _ = std::fs::remove_dir_all(&root);
}

/// Launched from inside a Herdr pane, the companion inherits that pane's
/// `HERDR_*` context. The isolated server must not: Herdr 0.9.5 turns an
/// inherited `HERDR_STARTUP_CWD` into an extra startup workspace.
/// The person's own `HERDR_CONFIG_PATH` is not pane context and stays.
#[test]
fn create_starts_the_server_without_the_inherited_herdr_pane_context() {
    let root = std::env::temp_dir().join(format!(
        "uml herdr env {} {}",
        std::process::id(),
        posix::monotonic_ms()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let exe = root.join("claude");
    std::fs::write(&exe, "").unwrap();
    let state: Shared = Rc::new(RefCell::new(FakeState {
        empty_sessions: true,
        cwd: Some(root.to_string_lossy().into_owned()),
        ..FakeState::default()
    }));
    let calls: Calls = Rc::new(RefCell::new(Vec::new()));
    let checkpoints: Calls2 = Rc::new(RefCell::new(Vec::new()));
    let recovered: Rc<RefCell<Option<Value>>> = Rc::new(RefCell::new(None));
    let mut backend = create_backend(state, calls, checkpoints, recovered);
    backend.options.environment = Rc::new(|| {
        strings(&[
            "PATH",
            "HERDR_STARTUP_CWD",
            "HERDR_PANE_ID",
            "HERDR_SOCKET_PATH",
            "HERDR_CONFIG_PATH",
            "XDG_CONFIG_HOME",
        ])
    });
    let removed: Rc<RefCell<Option<Vec<String>>>> = Rc::new(RefCell::new(None));
    let seen = removed.clone();
    backend.options.spawn_server = Some(Rc::new(move |_, _, _, remove| {
        *seen.borrow_mut() = Some(remove.to_vec());
        Ok(Some(Owner {
            pid: 41,
            start: "server-start".to_owned(),
            name: "herdr".to_owned(),
        }))
    }));
    backend
        .create(
            "uml-viewer-A",
            &root.to_string_lossy(),
            &[exe.to_string_lossy().into_owned()],
        )
        .unwrap();
    assert_eq!(
        Some(strings(&[
            "HERDR_STARTUP_CWD",
            "HERDR_PANE_ID",
            "HERDR_SOCKET_PATH"
        ])),
        removed.borrow().clone()
    );
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn cleanup_resumes_after_workspace_closure_only_under_the_same_server_identity() {
    let calls: Calls = Rc::new(RefCell::new(Vec::new()));
    let state: Shared = Rc::new(RefCell::new(FakeState {
        fail: Some(("session".to_owned(), "stop".to_owned())),
        ..FakeState::default()
    }));
    let checkpoint: Rc<RefCell<Option<Value>>> = Rc::new(RefCell::new(None));
    let checkpoint_into = checkpoint.clone();
    let checkpoint_out = checkpoint.clone();
    let run_state = state.clone();
    let run_calls = calls.clone();
    let backend = Herdr::new(
        Some(owned()),
        Options {
            run: Rc::new(move |argv, timeout| {
                fake_cli(run_calls.clone(), run_state.clone())(argv, timeout)
            }),
            owner: Rc::new(fake_owner(state.clone())),
            clock: Rc::new(|| 0),
            sleep: Rc::new(|_| {}),
            canonical: Rc::new(|path| path.to_owned()),
            selected: Some(Selected {
                path: "herdr".to_owned(),
                release: "0.9.3".to_owned(),
            }),
            write_companion: Rc::new(move |_, record| {
                *checkpoint_into.borrow_mut() = Some(record.clone());
                Ok(())
            }),
            read_companion: Rc::new(move |_| Ok(checkpoint_out.borrow().clone())),
            ..Options::real()
        },
    );
    assert!(backend.cleanup("uml-viewer-A").is_err());
    assert_eq!(
        Some("server-stopping".to_owned()),
        checkpoint
            .borrow()
            .as_ref()
            .and_then(|record| record::get_name(record, "cleanup-phase"))
    );
    // The stop failure cleared, the journaled checkpoint resumes the cleanup
    // and the workspace close is not repeated.
    state.borrow_mut().fail = None;
    let close_count = calls.clone();
    backend.cleanup("uml-viewer-A").unwrap();
    let closes = ops(&close_count)
        .iter()
        .filter(|op| *op == "workspace close")
        .count();
    assert_eq!(1, closes);
}

#[test]
fn cleanup_cancels_recovery_before_destroying_any_resource() {
    let mut record = owned();
    record::put(
        &mut record,
        "supervisor",
        record::map(vec![("token", Value::String("controller".to_owned()))]),
    );
    let order: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let order_run = order.clone();
    let order_cancel = order.clone();
    let calls: Calls = Rc::new(RefCell::new(Vec::new()));
    let state: Shared = Rc::new(RefCell::new(FakeState::default()));
    let run_state = state.clone();
    let backend = Herdr::new(
        Some(record.clone()),
        Options {
            run: Rc::new(move |argv, timeout| {
                order_run.borrow_mut().push(format!(
                    "{} {}",
                    argv[3],
                    argv.get(4).cloned().unwrap_or_default()
                ));
                fake_cli(calls.clone(), run_state.clone())(argv, timeout)
            }),
            owner: Rc::new(fake_owner(state.clone())),
            clock: Rc::new(|| 0),
            sleep: Rc::new(|_| {}),
            canonical: Rc::new(|path| path.to_owned()),
            selected: Some(Selected {
                path: "herdr".to_owned(),
                release: "0.9.3".to_owned(),
            }),
            write_companion: Rc::new(|_, _| Ok(())),
            read_companion: Rc::new(|_| Ok(None)),
            cancel_recovery: Some(Rc::new(move |_| {
                order_cancel.borrow_mut().push("cancel".to_owned());
                Ok(())
            })),
            ..Options::real()
        },
    );
    backend.cleanup("uml-viewer-A").unwrap();
    assert_eq!("cancel", order.borrow()[0]);

    let backend = Herdr::new(
        Some(record),
        Options {
            run: Rc::new(|_, _| panic!("must not execute")),
            owner: Rc::new(|_| None),
            clock: Rc::new(|| 0),
            sleep: Rc::new(|_| {}),
            canonical: Rc::new(|path| path.to_owned()),
            selected: Some(Selected {
                path: "herdr".to_owned(),
                release: "0.9.3".to_owned(),
            }),
            write_companion: Rc::new(|_, _| Ok(())),
            read_companion: Rc::new(|_| Ok(None)),
            cancel_recovery: Some(Rc::new(|_| Err(Failure::new("cannot confirm release")))),
            ..Options::real()
        },
    );
    assert!(backend.cleanup("uml-viewer-A").is_err());
}

/// Relaunch: the reset sequence runs before ctrl+c before the submission, a
/// failed reset stops the relaunch, and an occupied foreground refuses.
mod relaunch {
    use super::*;

    type RunOverride = Rc<dyn Fn(&[String], Option<i64>) -> RunOut>;

    fn identity() -> Value {
        let mut identity = owned();
        record::put(
            &mut identity,
            "runner-token",
            Value::String("gen-token".to_owned()),
        );
        let directory = std::env::temp_dir().join(format!(
            "uml herdr relaunch {} {}",
            std::process::id(),
            crate::posix::monotonic_ms()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        record::put(
            &mut identity,
            "runner-state",
            Value::String(directory.join("runner.edn").to_string_lossy().into_owned()),
        );
        identity
    }

    fn launch_with(state: Shared, run_override: Option<RunOverride>) -> Result<(Calls, Value)> {
        let calls: Calls = Rc::new(RefCell::new(Vec::new()));
        let run_calls = calls.clone();
        let run_state = state.clone();
        let identity = identity();
        let run = run_override.unwrap_or_else(|| {
            Rc::new(move |argv: &[String], timeout: Option<i64>| {
                let text = argv.get(6).cloned().unwrap_or_default();
                let is_pane_run = argv.get(3).map(String::as_str) == Some("pane")
                    && argv.get(4).map(String::as_str) == Some("run");
                if is_pane_run && text.contains("printf") && run_state.borrow().reset_fail {
                    return RunOut {
                        exit: Some(4),
                        out: String::new(),
                        err: "nonexistent directory".to_owned(),
                        timeout: false,
                    };
                }
                if is_pane_run {
                    run_state.borrow_mut().run_text = Some(text.clone());
                }
                fake_cli(run_calls.clone(), run_state.clone())(argv, timeout)
            })
        });
        let mut backend = Herdr::new(
            Some(identity.clone()),
            Options {
                run,
                owner: Rc::new(fake_owner(state.clone())),
                clock: Rc::new(|| 0),
                sleep: Rc::new(|_| {}),
                canonical: Rc::new(|path| path.to_owned()),
                selected: Some(Selected {
                    path: "herdr".to_owned(),
                    release: "0.9.3".to_owned(),
                }),
                write_companion: Rc::new(|_, _| Ok(())),
                read_companion: Rc::new(|_| Ok(None)),
                runner: Some(RunnerLauncher {
                    argv: vec![
                        "love".to_owned(),
                        "/game".to_owned(),
                        "companion-runner".to_owned(),
                    ],
                }),
                ..Options::real()
            },
        );
        backend.relaunch(&identity)?;
        Ok((calls, identity))
    }

    #[test]
    fn resets_killed_tui_terminal_modes_and_the_input_line_before_submitting() {
        let state: Shared = Rc::new(RefCell::new(FakeState::default()));
        let (calls, _) = launch_with(state.clone(), None).unwrap();
        let mut reset_index = None;
        let mut keys_index = None;
        let mut run_index = None;
        for (index, argv) in calls.borrow().iter().enumerate() {
            let op = format!("{} {}", argv[3], argv.get(4).cloned().unwrap_or_default());
            let text = argv.get(6).cloned().unwrap_or_default();
            if op == "pane run" && text.contains("printf") {
                reset_index = Some(index);
            } else if op == "pane send-keys" {
                keys_index = Some(index);
            } else if op == "pane run" && text.contains("companion-runner") {
                run_index = Some(index);
            }
        }
        let (reset, keys, run) = (
            reset_index.expect("reset"),
            keys_index.expect("keys"),
            run_index.expect("run"),
        );
        assert!(reset < keys && keys < run);
        let script = calls.borrow()[reset].join(" ");
        for mode in [
            "[?1049l", "[?2004l", "[<u", "[?1000l", "[?1002l", "[?1003l", "[?1006l",
        ] {
            assert!(script.contains(mode), "reset must clear {mode}");
        }
        let text = state.borrow().run_text.clone().expect("runner submitted");
        assert!(text.contains("'companion-runner'"), "{text}");
    }

    #[test]
    fn fails_closed_without_submitting_when_the_terminal_reset_fails() {
        let state: Shared = Rc::new(RefCell::new(FakeState {
            reset_fail: true,
            ..FakeState::default()
        }));
        assert!(launch_with(state.clone(), None).is_err());
        let text = state.borrow().run_text.clone();
        assert!(
            text.is_none()
                || !text
                    .as_deref()
                    .unwrap_or_default()
                    .contains("companion-runner"),
            "the replacement command must never be submitted"
        );
    }

    #[test]
    fn refuses_submission_when_the_pane_foreground_is_unknown_or_occupied() {
        for foreground in [
            None,
            Some(Json::Array(vec![
                object(vec![
                    ("pid", Json::Number(42.0)),
                    ("name", Json::String("bash".to_owned())),
                ]),
                object(vec![
                    ("pid", Json::Number(7.0)),
                    ("name", Json::String("claude".to_owned())),
                ]),
            ])),
        ] {
            let state: Shared = Rc::new(RefCell::new(FakeState {
                foreground_absent: foreground.is_none(),
                foreground,
                ..FakeState::default()
            }));
            assert!(launch_with(state.clone(), None).is_err());
            let text = state.borrow().run_text.clone();
            assert!(
                text.is_none()
                    || !text
                        .as_deref()
                        .unwrap_or_default()
                        .contains("companion-runner"),
                "the replacement command must never be submitted"
            );
        }
    }

    #[test]
    fn retries_only_canonical_mode_rejections_of_the_submitted_command() {
        let state: Shared = Rc::new(RefCell::new(FakeState::default()));
        let attempts = Rc::new(RefCell::new(0));
        let counted = attempts.clone();
        let run_state = state.clone();
        let calls: Calls = Rc::new(RefCell::new(Vec::new()));
        let run_calls = calls.clone();
        let run: RunOverride = Rc::new(move |argv, timeout| {
            let op = (
                argv.get(3).map(String::as_str),
                argv.get(4).map(String::as_str),
            );
            let text = argv.get(6).cloned().unwrap_or_default();
            if op == (Some("pane"), Some("run")) {
                if text.contains("printf") {
                    return response(object(vec![(
                        "result",
                        object(vec![("type", Json::String("ok".to_owned()))]),
                    )]));
                }
                *counted.borrow_mut() += 1;
                if *counted.borrow() < 3 {
                    return RunOut {
                        exit: Some(1),
                        out: String::new(),
                        err: "pane_send_failed: input exceeds the canonical mode limit".to_owned(),
                        timeout: false,
                    };
                }
                run_state.borrow_mut().run_text = Some(text);
                return response(object(vec![(
                    "result",
                    object(vec![("type", Json::String("ok".to_owned()))]),
                )]));
            }
            fake_cli(run_calls.clone(), run_state.clone())(argv, timeout)
        });
        launch_with(state, Some(run)).unwrap();
        assert_eq!(3, *attempts.borrow());
    }
}
