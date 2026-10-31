//! Pure read-only preflight rules (port of `love/uml/domain/companion.lua`):
//! the backend a recorded ownership names, the `--backend` argument, and the
//! validation that runs before any Herdr command or resource creation.

use crate::record;
use crate::vocab::{Failure, Result};
use edn::Value;

pub const LEGACY_GUIDANCE: &str = "tmux/psmux companions are retired. Ownership and resources were retained; no takeover or cleanup was attempted. Finish the old viewer/session using the old release, verify that this project's legacy resources have exited, then archive .uml-viewer/companion.edn and start ./uml fresh with Herdr. Do not use --restart to migrate; retain session.edn and mailbox queues.";

/// The backend named by a present ownership record. Legacy (tmux/psmux) or
/// unmarked records are retained and refused.
pub fn recorded_backend(record: Option<&Value>) -> Result<Option<String>> {
    let Some(record) = record else {
        return Ok(None);
    };
    if record::is_empty_map(record) {
        return Ok(None);
    }
    if !record::is_map(record) {
        return Ok(None);
    }
    let backend = record::get_name(record, "backend");
    match backend.as_deref() {
        None | Some("tmux") | Some("psmux") => {
            Err(Failure::with_reason(LEGACY_GUIDANCE, "legacy-ownership"))
        }
        Some(backend) => Ok(Some(backend.to_owned())),
    }
}

/// Removes only the exact lowercase `--backend herdr` pair; all other argv is
/// retained literally (an uppercase `--BACKEND` is not an option).
pub fn selection_args(args: &[String]) -> Result<(Vec<String>, Option<String>)> {
    let mut forwarded = Vec::new();
    let mut selected: Option<String> = None;
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        if arg == "--backend" {
            let value = args.get(index + 1).map(String::as_str);
            if matches!(value, Some("tmux") | Some("psmux")) {
                return Err(Failure::with_reason(LEGACY_GUIDANCE, "retired-backend"));
            }
            if selected.is_some() || value != Some("herdr") {
                return Err(Failure::new(
                    "Unknown or duplicate companion backend; use --backend herdr",
                ));
            }
            selected = Some("herdr".to_owned());
            index += 2;
        } else {
            forwarded.push(arg.clone());
            index += 1;
        }
    }
    Ok((forwarded, selected))
}

pub struct PreflightOptions<'a> {
    pub root: &'a str,
    pub requested: Option<String>,
    pub restart: bool,
    /// The delivered-platform decision: `Ok(())` supported, `Err(reason)`.
    pub platform: std::result::Result<(), String>,
    pub canonical: &'a dyn Fn(&str) -> String,
}

/// Read-only validation before any Herdr command or resource creation.
pub fn preflight_checks(record: Option<&Value>, opts: &PreflightOptions) -> Result<String> {
    if let Some(record) = record
        && !record::is_map(record)
    {
        return Err(Failure::new(
            "Present companion ownership is corrupt or unreadable; file retained",
        ));
    }
    let recorded = recorded_backend(record)?;
    if let Some(recorded) = &recorded
        && recorded != "herdr"
    {
        return Err(Failure::new(
            "Unknown recorded companion backend; ownership retained",
        ));
    }
    if let Some(record) = record
        && record::is_map(record)
        && !record::is_empty_map(record)
    {
        let complete = [
            "socket",
            "workspace",
            "terminal-id",
            "shell-pid",
            "owner-start",
            "server-pid",
            "server-start",
        ]
        .iter()
        .all(|key| edn::record::get(record, key).is_some());
        if !complete
            || record::get_name(record, "session").is_none_or(|session| session.is_empty())
            || record::get_name(record, "pane").is_none_or(|pane| pane.is_empty())
            || edn::record::get(record, "cwd").is_none()
            || record::get_name(record, "owner-role").as_deref() != Some("companion")
            || edn::record::is_true(record, "provisional")
        {
            return Err(Failure::new(
                "Companion ownership is incomplete, provisional or belongs to another project; file retained",
            ));
        }
        let cwd = record::get_name(record, "cwd").unwrap_or_default();
        if (opts.canonical)(opts.root) != (opts.canonical)(&cwd) {
            return Err(Failure::new(
                "Companion ownership is incomplete, provisional or belongs to another project; file retained",
            ));
        }
    }
    if let Some(requested) = &opts.requested
        && requested != "herdr"
    {
        return Err(Failure::new(LEGACY_GUIDANCE));
    }
    if opts.restart && recorded.is_none() {
        return Err(Failure::new(
            "Companion ownership is missing; cannot restart",
        ));
    }
    if let Err(reason) = &opts.platform {
        return Err(Failure::new(format!(
            "{reason}; ownership and queues are retained."
        )));
    }
    Ok(recorded.unwrap_or_else(|| "herdr".to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_guidance_is_ported_verbatim() {
        assert!(LEGACY_GUIDANCE.contains("Do not use --restart to migrate"));
    }

    #[test]
    fn selection_removes_only_the_exact_lowercase_backend_option() {
        let (args, backend) = selection_args(&[
            "--BACKEND".into(),
            "custom.edn".into(),
            "--backend".into(),
            "herdr".into(),
            "--restart".into(),
        ])
        .unwrap();
        assert_eq!(vec!["--BACKEND", "custom.edn", "--restart"], args);
        assert_eq!(Some("herdr".to_owned()), backend);
    }

    #[test]
    fn selection_refuses_retired_unknown_and_duplicate_backends() {
        for args in [vec!["--backend", "tmux"], vec!["--backend", "psmux"]] {
            let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            let error = selection_args(&args).unwrap_err();
            assert_eq!(Some("retired-backend"), error.reason);
        }
        for args in [
            vec!["--backend", "invalid"],
            vec!["--backend"],
            vec!["--backend", "herdr", "--backend", "herdr"],
        ] {
            let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
            let error = selection_args(&args).unwrap_err();
            assert!(
                error
                    .message
                    .contains("Unknown or duplicate companion backend")
            );
        }
    }

    fn preflight(record: Option<&Value>, requested: Option<&str>, restart: bool) -> Result<String> {
        preflight_checks(
            record,
            &PreflightOptions {
                root: "/project",
                requested: requested.map(str::to_owned),
                restart,
                platform: Ok(()),
                canonical: &(|path| path.to_owned()),
            },
        )
    }

    #[test]
    fn preflight_accepts_fresh_launch_and_complete_ownership() {
        assert_eq!("herdr", preflight(None, None, false).unwrap());
        let record = edn::parse(
            r#"{:backend :herdr :owner-role :companion :cwd "/project" :session "uml-viewer-A"
                :pane "w1:p1" :socket "/s/herdr.sock" :workspace "w1" :terminal-id "term_A"
                :shell-pid 42 :owner-start "ss" :server-pid 41 :server-start "rs"}"#,
        )
        .unwrap();
        assert_eq!("herdr", preflight(Some(&record), None, false).unwrap());
    }

    #[test]
    fn preflight_refuses_legacy_and_unmarked_ownership_with_guidance() {
        for backend in [Some(":tmux"), Some(":psmux"), None] {
            for restart in [false, true] {
                let text = format!(
                    "{{{} :cwd \"/project\" :session \"saved\" :pane \"%1\"}}",
                    backend.map(|b| format!(":backend {b}")).unwrap_or_default()
                );
                let record = edn::parse(&text).unwrap();
                let error = preflight(Some(&record), None, restart).unwrap_err();
                assert_eq!(Some("legacy-ownership"), error.reason);
                assert!(error.message.contains("old release"));
                assert!(error.message.contains("Do not use --restart to migrate"));
            }
        }
    }

    #[test]
    fn preflight_refuses_unknown_backends_retired_requests_and_unsupported_platforms() {
        let record = edn::parse(r#"{:backend :other :owner-role :companion}"#).unwrap();
        let error = preflight(Some(&record), None, false).unwrap_err();
        assert!(error.message.contains("Unknown recorded companion backend"));
        for requested in ["tmux", "psmux", "unknown"] {
            let error = preflight(None, Some(requested), false).unwrap_err();
            assert!(error.message.contains("companions are retired"));
        }
        let refused = preflight_checks(
            None,
            &PreflightOptions {
                root: "/project",
                requested: None,
                restart: false,
                platform: Err("Host refused".to_owned()),
                canonical: &(|path| path.to_owned()),
            },
        )
        .unwrap_err();
        assert_eq!(
            "Host refused; ownership and queues are retained.",
            refused.message
        );
    }

    #[test]
    fn preflight_rejects_missing_restart_corrupt_and_incomplete_or_foreign_records() {
        let missing = preflight(None, None, true).unwrap_err();
        assert!(missing.message.contains("ownership is missing"));
        let corrupt = edn::parse("[:invalid]").unwrap();
        let error = preflight(Some(&corrupt), None, false).unwrap_err();
        assert!(error.message.contains("corrupt or unreadable"));
        let valid = r#"{:backend :herdr :owner-role :companion :cwd "/project" :session "saved"
            :pane "%1" :socket "/s" :workspace "w1" :terminal-id "t" :shell-pid 1
            :owner-start "o" :server-pid 2 :server-start "r"}"#;
        for changed in [
            valid.replace(":owner-role :companion", ":owner-role :launcher"),
            valid.replace(":session \"saved\"", ":session \"\""),
            valid.replace(":cwd \"/project\"", ":cwd \"/other\""),
            valid.replace("}", ":provisional true}"),
            valid.replace(":terminal-id \"t\" ", ""),
        ] {
            let record = edn::parse(&changed).unwrap();
            let error = preflight(Some(&record), None, true).unwrap_err();
            assert!(
                error
                    .message
                    .contains("incomplete, provisional or belongs to another project"),
                "{changed}: {}",
                error.message
            );
        }
    }
}
