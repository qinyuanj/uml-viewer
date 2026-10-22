//! Seam A (#134): the binary's process boundary. Each role named in #134
//! is a subcommand; roles not yet ported exit 69 (EX_UNAVAILABLE) naming the
//! role, unknown `--` options are refused with usage (exit 2).

use std::process::{Command, Output};

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_uml-viewer"))
        .args(args)
        .output()
        .unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn every_role_from_the_spec_is_a_subcommand() {
    for role in [
        "companion-worker",
        "companion-runner",
        "companion-controller",
        "companion-preflight",
        "companion-smoke",
    ] {
        let output = run(&[role, "some arg"]);
        assert_ne!(
            Some(2),
            output.status.code(),
            "{role} is an unknown subcommand: {}",
            stderr(&output)
        );
    }
}

#[test]
fn companion_roles_are_ported_and_report_real_failures() {
    // #138: the five companion roles dispatch to the companion crate, so a
    // bogus argument is a named failure (never the unported placeholder).
    for role in [
        "companion-worker",
        "companion-runner",
        "companion-controller",
        "companion-preflight",
        "companion-smoke",
    ] {
        let output = run(&[role, "some arg"]);
        assert_ne!(
            Some(69),
            output.status.code(),
            "{role} is ported but still reported as unported"
        );
        assert!(
            !stderr(&output).contains("尚未移植到 Rust 运行时"),
            "{role}: {}",
            stderr(&output)
        );
    }
}

#[test]
fn ir_is_ported_and_reports_bad_arguments_on_stderr() {
    // #136: the ir role runs headless; an unknown option is a usage failure
    // (exit 1) naming the option, not EX_UNAVAILABLE.
    let output = run(&["ir", "--bogus"]);
    assert_eq!(Some(1), output.status.code());
    let err = stderr(&output);
    assert!(err.contains("IR generation failed"), "{err}");
    assert!(err.contains("--bogus"), "{err}");
}

#[test]
fn arguments_without_a_subcommand_go_to_the_viewer() {
    // #139 ports the viewer, so it is reached instead of EX_UNAVAILABLE. A
    // real positional path would open a window and block the test; an
    // option-looking argument exercises the same dispatch and fails usage.
    let output = run(&["-x"]);
    assert_eq!(Some(2), output.status.code());
    assert!(
        stderr(&output).contains("usage: uml-viewer viewer"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn viewer_is_ported_and_reports_usage_errors_without_opening_a_window() {
    // #139: the viewer role parses argv before creating a window.
    let output = run(&["viewer", "--bogus"]);
    assert_eq!(Some(2), output.status.code());
    assert!(
        stderr(&output).contains("usage: uml-viewer viewer"),
        "{}",
        stderr(&output)
    );
    let output = run(&["viewer", "--backend", "tmux"]);
    assert_eq!(Some(2), output.status.code());
    assert!(
        stderr(&output).contains("Unknown or duplicate companion backend"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn help_lists_the_subcommands_and_succeeds() {
    let output = run(&["--help"]);
    assert_eq!(Some(0), output.status.code());
    let text = String::from_utf8_lossy(&output.stdout);
    for role in ["viewer", "ir", "scene", "companion-preflight"] {
        assert!(text.contains(role), "{text}");
    }
}

#[test]
fn an_unknown_leading_option_is_refused_with_usage() {
    let output = run(&["--bogus"]);
    assert_eq!(Some(2), output.status.code());
    assert!(stderr(&output).contains("usage"), "{}", stderr(&output));
}

#[test]
fn scene_is_ported_and_reports_usage_errors() {
    // #137: no `--ir` is a usage error (exit 2), not "unported" (69).
    let output = run(&["scene"]);
    assert_eq!(Some(2), output.status.code());
    assert!(stderr(&output).contains("usage"), "{}", stderr(&output));
    let output = run(&["scene", "--bogus"]);
    assert_eq!(Some(2), output.status.code());
    assert!(stderr(&output).contains("usage"), "{}", stderr(&output));
    // A missing IR file is a runtime failure (exit 1), named on stderr.
    let output = run(&["scene", "--ir", "no-such-file.edn"]);
    assert_eq!(Some(1), output.status.code());
    assert!(
        stderr(&output).contains("no-such-file.edn"),
        "{}",
        stderr(&output)
    );
}
