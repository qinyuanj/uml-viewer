//! Read-only companion backend preflight (port of
//! `love/uml/adapters/companion_preflight.lua` and the `companion-preflight`
//! role). Never creates a resource; selection is pure and the adapter only
//! supplies PATH candidates, executable probing, ownership bytes and commands.

use crate::herdr::select::{self, Probe, Selected};
use crate::ownership;
use crate::posix;
use crate::vocab::{Result, RunOut};

/// PATH candidates, in PATH order, deduplicated by the candidate path.
fn candidates(path: &str) -> Vec<String> {
    let probe = |candidate: &str| {
        std::fs::File::open(candidate).ok().map(|_| Probe {
            executable: true,
            canonical: candidate.to_owned(),
        })
    };
    select::herdr_candidates(path, &probe)
}

fn run_herdr(argv: &[String], timeout_ms: Option<i64>) -> RunOut {
    let os_argv: Vec<std::ffi::OsString> = argv.iter().map(std::ffi::OsString::from).collect();
    posix::run(&os_argv, timeout_ms, &[])
}

/// The full read-only preflight: ownership, requested backend, platform, then
/// Herdr selection. Returns the selected Herdr.
pub fn select(root: &str, args: &[String], restart: bool) -> Result<Selected> {
    let path = std::env::var("PATH").unwrap_or_default();
    select_on_path(root, args, restart, &path)
}

/// Selection against an explicit PATH; the role passes the process PATH and
/// tests pass a fixture directory, so no test mutates process state.
pub fn select_on_path(root: &str, args: &[String], restart: bool, path: &str) -> Result<Selected> {
    let (_, requested) = rules::selection_args(args)?;
    let record = ownership::read(root)?;
    let canonical = |path: &str| posix::canonical(path).unwrap_or_else(|_| path.to_owned());
    rules::preflight_checks(
        record.as_ref(),
        &rules::PreflightOptions {
            root,
            requested,
            restart,
            platform: posix::platform_support(),
            canonical: &canonical,
        },
    )?;
    let candidates = candidates(path);
    select::select_herdr(&candidates, &run_herdr)
}

/// The `companion-preflight` role: stdout is exactly `herdr`, stderr carries
/// the selected Herdr, exit 0; failures are one `uml: ...` line and exit 1.
pub fn run(args: &[std::ffi::OsString]) -> std::process::ExitCode {
    let root = args
        .first()
        .map(|root| root.to_string_lossy().into_owned())
        .unwrap_or_default();
    let rest: Vec<String> = args[1..]
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    match select(&root, &rest, false) {
        Ok(selected) => {
            println!("herdr");
            eprintln!(
                "uml: selected Herdr {} at {}",
                selected.release, selected.path
            );
            std::process::ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("uml: {}", error.message);
            std::process::ExitCode::FAILURE
        }
    }
}

pub(crate) mod rules;

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root() -> String {
        static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "uml preflight {} {}",
            std::process::id(),
            SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(directory.join(".uml-viewer")).unwrap();
        directory.to_string_lossy().into_owned()
    }

    fn write_record(root: &str, text: &str) {
        std::fs::write(
            std::path::Path::new(root).join(".uml-viewer/companion.edn"),
            text,
        )
        .unwrap();
    }

    #[test]
    fn a_fresh_launch_selects_herdr_without_creating_ownership() {
        let root = temp_root();
        // PATH points at a fixture directory with a compatible herdr.
        let bin = std::path::Path::new(&root).join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let herdr = bin.join("herdr");
        std::fs::write(
            &herdr,
            "#!/bin/sh\nif [ \"$1\" = --version ]; then echo 'herdr 0.9.4'; exit 0; fi\n"
                .to_owned()
                + "echo '{\"$schema\":\"fixture\",\"protocol\":22,\"schema_version\":1}'\n",
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&herdr).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
        std::fs::set_permissions(&herdr, permissions).unwrap();
        let path = format!("{}:/definitely/empty", bin.display());
        let selected = select_on_path(
            &root,
            &["--backend".into(), "herdr".into(), "d.edn".into()],
            false,
            &path,
        )
        .unwrap();
        assert!(selected.path.ends_with("/bin/herdr"));
        assert_eq!("0.9.4", selected.release);
        assert!(!ownership::exists(&root));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn legacy_unmarked_unknown_and_incomplete_ownership_is_refused_before_selection() {
        let patterns = [
            r#"{:backend :tmux :cwd "%s" :session "saved" :pane "%1"}"#,
            r#"{:cwd "%s" :session "saved" :pane "%1"}"#,
            r#"{:backend :unknown :cwd "%s" :session "saved" :pane "%1"}"#,
            r#"{:backend :herdr :owner-role :companion :cwd "%s" :session "saved" :pane "%1"}"#,
            r#"{:backend :herdr :owner-role :launcher :cwd "%s" :session "saved" :pane "%1"}"#,
            r#"{:backend :herdr :owner-role :companion :cwd "%s" :session "saved" :pane "%1" :provisional true}"#,
        ];
        for pattern in patterns {
            let root = temp_root();
            write_record(&root, &pattern.replace("%s", &root));
            let before = ownership::bytes(&root);
            let error = select(&root, &[], false).unwrap_err();
            assert!(!error.message.is_empty());
            assert_eq!(before, ownership::bytes(&root), "bytes are retained");
            let _ = std::fs::remove_dir_all(&root);
        }
    }

    #[test]
    fn corrupt_ownership_is_retained_and_a_missing_restart_is_refused() {
        let root = temp_root();
        write_record(&root, "{:backend :herdr");
        let error = select(&root, &[], false).unwrap_err();
        assert!(error.message.contains("corrupt or unreadable"));
        std::fs::remove_file(std::path::Path::new(&root).join(".uml-viewer/companion.edn"))
            .unwrap();
        let missing = select(&root, &["--restart".into()], true).unwrap_err();
        assert!(missing.message.contains("ownership is missing"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn every_incompatible_path_candidate_is_reported() {
        let root = temp_root();
        let bin = std::path::Path::new(&root).join("a");
        std::fs::create_dir_all(&bin).unwrap();
        let herdr = bin.join("herdr");
        std::fs::write(&herdr, "#!/bin/sh\necho 'herdr 0.9.2'\n").unwrap();
        let mut permissions = std::fs::metadata(&herdr).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o755);
        std::fs::set_permissions(&herdr, permissions).unwrap();
        let path = format!("{}:/definitely/empty", bin.display());
        let error = select_on_path(&root, &[], false, &path).unwrap_err();
        assert!(
            error.message.contains("Candidates on PATH"),
            "{}",
            error.message
        );
        assert!(
            error
                .message
                .contains("(unknown: not a herdr X.Y.Z >= 0.9.3)")
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn the_platform_decision_is_the_delivered_wsl_linux_target() {
        // On the delivered host this is Ok; elsewhere the refusal reason is
        // the ADR 0007 text. Either way it must not be empty.
        if let Err(reason) = posix::platform_support() {
            assert!(
                reason.contains("supported platform")
                    || reason.contains("Unsupported companion platform")
            );
        }
    }

    #[test]
    fn failure_messages_are_single_lines_for_the_entry() {
        let error = crate::vocab::Failure::new("one line");
        assert!(!error.message.contains('\n'));
    }
}
