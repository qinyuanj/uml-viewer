//! Pure companion policy: executable selection, read-only ownership preflight
//! and the verbatim prompts handed to Claude (port of
//! `love/uml/domain/companion.lua`). No filesystem or process access.

use crate::record;
use crate::vocab::{Failure, Result, RunOut};
use edn::Value;

pub const LEGACY_GUIDANCE: &str = "tmux/psmux companions are retired. Ownership and resources were retained; no takeover or cleanup was attempted. Finish the old viewer/session using the old release, verify that this project's legacy resources have exited, then archive .uml-viewer/companion.edn and start ./uml fresh with Herdr. Do not use --restart to migrate; retain session.edn and mailbox queues.";
pub const HERDR_GUIDANCE: &str = "Herdr 0.9.3 or newer (protocol 22) is required. Install a release from https://github.com/herdrdev/herdr/releases (https://herdr.dev). No automatic download, upgrade or fallback. WSL2 + WSLg is the delivered platform; Windows native is unsupported and macOS is not committed (ADR 0007). On WSL, terminal attach opens Windows Terminal; its manual acceptance is pending (#114).";
pub const RECOVERY_PROMPT: &str = "Recovery: do not scan sources or rewrite policy. Read the current .uml-viewer/to-agent.edn queue; pop and handle commands oldest first, preserving the latest context and existing proposals. Apply the standing quality-result gate to each refresh; only valid results followed by successful IR permit :display. A failed refresh stops that request's IR/display, reports its cause, and leaves later FIFO mail runnable. If there is no pending refresh, restore :display from .uml-viewer/view.edn; never issue an unconditional recovery display after a failed refresh. Then wait for directives.";
pub const STANDING_RULES: &str = r#"You are the UML-viewer companion. The current working directory is the
project being examined (the one whose diagram is on screen), not
necessarily uml-viewer itself.
On launch: inspect this directory's source, write or update a
hierarchical policy from the real namespaces (dots after the ns prefix
are the tree; do not invent Domain/Engine/Adapters packages), then
regenerate the IR (`./uml ir`) so the
diagram matches. Do not edit the generated EDN by hand.
After every later source or policy change:
1. Keep the policy as namespace nesting only. Do not re-home a ns to
   fake a layer/component. Layer and component mean the same thing.
   If the tree is wrong, change the requires or the ns.
   Preserve :proposals (named components that are not namespaces). Do not
   invent :proposals on launch. The inspector lists them; click the real
   diagram above Proposals to return to the namespace tree. If instructed,
   add a named proposal to :proposals in the policy (default name is a
   timestamp) and regenerate the IR.
   Edit the policy file in place: change only the keys you mean to
   change and keep its comments, ordering and line layout. Never
   re-serialize the whole policy; its comments carry the reasons.
TypeScript, Rust, Python, and Lua are the supported scanners. One language sets
:lang for the whole tree; more than one language sets :sources, each
entry {:lang :root :prefix?}. The policy :prefix is stripped to make
ids. A source :prefix is that tree's namespace root, for example
bookwriter.rust beside TypeScript modules under bookwriter. Dots in the
module id are still the tree. Do not invent packages. A TypeScript
invoke("name") depends on the Rust #[tauri::command] of that name.
Python is one class per .py module. import and from-import are dependencies.
class Child(Base) is inheritance when Base is a project module. A base of
Protocol or ABC is implements.
Lua is one class per .lua module. require("a.b") is a dependency on the
a.b module. Non-local functions are the operations. A top-level
setmetatable(t, {__index = Base}) or setmetatable(t, Base), with Base
from local Base = require("m"), is inheritance from m.
Snapshots join by the class :ns, then by the class id.
./uml ir runs the Rust IR generator on this project's policy. Use the public
./uml crap and ./uml mutate quality dispatch for mailbox refreshes: it
shares CLI routing, optional uml-quality.json overrides and standard tool
scope/exclusions. If the entry is absent, install it
with get-uml-viewer --install-only before refreshing; direct tool commands
do not publish a result record.
2. Run ./uml crap with declared scope: standard tools cover the whole
   business tree; an operation override may declare targets scope.
   Preserve unrelated class metrics under the selected tool contract.
3. Run one ./uml mutate call on every changed source file
   (differential); it shares one coverage run and baseline and
   publishes each file's snapshot.
   Retain explicit targets; never inject --source-root on scoped mutation.
   Uncovered mutants and survivors are findings: keep the snapshot;
   do not re-run the file or pass --mutate-all because of findings.
4. Serialize quality invocations. Before each, remove any previous
   .uml-viewer/quality-result.json and note invocation start; await the
   command and capture its actual exit. Read only its newly published
   record: version 1, matching operation, canonical projectRoot equal
   to this project, exitCode equal to the actual process exit, and
   validResult exactly true. Missing, stale, malformed or mismatched
   records and validResult false stop this request's IR/display; report
   the cause. Exit 3 is valid only under the tool or explicit project
   operation contract, never by a universal success rule.
   Every invocation in a component must pass this gate before IR.
5. Only then run ./uml ir; only IR exit 0 permits :display. An IR
   failure stops display and reports the cause. Continue later FIFO
   requests after a failure; retain the latest discussion context.
Mailbox: .uml-viewer/to-agent.edn and to-viewer.edn are queues
{:next-id n :queue [cmd …]} (atomic: tmp then rename). Pop the head of
:queue as you handle it (rewrite the file). Oldest first. Ops are
{:id n :op :display :path "..."}, {:id n :op :regen},
{:id n :op :quit-for-restart}, {:id n :op :context ...},
and right-click element ops:
{:id n :op :refresh-crap :target {...}}, :refresh-mutate,
:refresh-mutate-all, :omit. :target is {:id :ns :kind :class|:component
 :proposal-id?}. For :refresh-crap follow the declared CRAP scope above,
 then apply the result gate before IR. Standard crapper gets no partial
 target paths. For
 :refresh-mutate run ./uml mutate on those source files
 (differential). For
 :refresh-mutate-all pass --mutate-all on those files. For :omit, if
 :proposal-id is set add :id to that proposal's :omit; otherwise add it
 to policy :omit. Then regenerate the IR.
:context means the inspector selection is the discussion context:
{:context :real} for the namespace tree, or {:context :proposal
 :proposal-id id :name "..."} for a named proposal. Treat that as
the architecture under discussion until a later :context arrives.
A companion wake-up means mail is waiting. If idle, pop and handle each
to-agent command in order. If busy, finish first. Do not control the session backend yourself.
After successful regen only, write :display with the generated EDN path.
Prefer ./uml (fresh start: spawn this companion, wait for :display)
and ./uml --restart (new Rust viewer, keep this session, restore last view).
The viewer is the Rust egui binary; there is no JVM, Clojure or LOVE
dependency. If ./uml is missing, tell the user to run get-uml-viewer in this
directory (it clones uml-viewer into gitignored .uml-viewer/uml-viewer/).
Do not start the viewer on launch; it reloads EDN when the file mtime
changes. To restart it: write :quit-for-restart, wait for the viewer to
exit, then ./uml --restart. Do not pass --restart except through that
wrapper. Do not SIGKILL. Closing the viewer
closes only this project's Herdr session. Verified abnormal exits receive
at most three retries (1, 2, 4 seconds); normal or unknown exits do not recover.
Recovery resumes this Claude session and drains queued mail without fresh scanning.
Do not commit or push unless asked.
"#;
pub const LAUNCH_PROMPT: &str = "On launch: from this working directory, update the hierarchical policy to match the project's namespaces (no invented layers/components), regenerate the IR, then wait for directives.";

/// Only dotted numeric X.Y.Z at or above 0.9.3 (ADR 0003).
pub fn herdr_version_supported(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    let mut numbers = [0u64; 3];
    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
        numbers[index] = match part.parse() {
            Ok(number) => number,
            Err(_) => return false,
        };
    }
    let floor = [0, 9, 3];
    for index in 0..3 {
        if numbers[index] != floor[index] {
            return numbers[index] > floor[index];
        }
    }
    true
}

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

pub struct Probe {
    pub executable: bool,
    pub canonical: String,
}

/// Distinct herdr executables in POSIX PATH order, deduplicated by canonical
/// path. Quoted or empty PATH entries are ignored.
pub fn herdr_candidates(path: &str, probe: &dyn Fn(&str) -> Option<Probe>) -> Vec<String> {
    let mut result = Vec::new();
    let mut seen = Vec::new();
    for part in path.split(':') {
        let part = part.replace('"', "");
        if part.is_empty() {
            continue;
        }
        let candidate = format!("{}/herdr", part.trim_end_matches('/'));
        if let Some(observed) = probe(&candidate)
            && observed.executable
            && !seen.contains(&observed.canonical)
        {
            seen.push(observed.canonical.clone());
            result.push(observed.canonical);
        }
    }
    result
}

/// The bundled schema text must open with `"$schema"` and declare
/// `"protocol": 22` before any later field (the same text pattern the Lua
/// preflight matched, not a full JSON parse).
fn schema_is_protocol_22(text: &str) -> bool {
    let bytes = text.as_bytes();
    let mut position = 0;
    let skip = |position: &mut usize| {
        while *position < bytes.len() && matches!(bytes[*position], b' ' | b'\t' | b'\r' | b'\n') {
            *position += 1;
        }
    };
    let expect = |position: &mut usize, literal: &str| -> bool {
        skip(position);
        if bytes[*position..].starts_with(literal.as_bytes()) {
            *position += literal.len();
            true
        } else {
            false
        }
    };
    let string = |position: &mut usize| -> bool {
        skip(position);
        if *position >= bytes.len() || bytes[*position] != b'"' {
            return false;
        }
        *position += 1;
        while *position < bytes.len() && bytes[*position] != b'"' {
            *position += 1;
        }
        if *position >= bytes.len() {
            return false;
        }
        *position += 1;
        true
    };
    expect(&mut position, "{")
        && expect(&mut position, "\"$schema\"")
        && expect(&mut position, ":")
        && string(&mut position)
        && expect(&mut position, ",")
        && expect(&mut position, "\"protocol\"")
        && expect(&mut position, ":")
        && expect(&mut position, "22")
        && expect(&mut position, ",")
}

pub struct ProbeResult {
    pub path: String,
    pub release: Option<String>,
    pub reason: Option<String>,
}

pub fn probe_herdr(run: &dyn Fn(&[String], Option<i64>) -> RunOut, exe: &str) -> ProbeResult {
    let argv = [exe.to_owned(), "--version".to_owned()];
    let version = run(&argv, None);
    let release = if version.exit == Some(0) {
        version
            .out
            .strip_prefix("herdr ")
            .filter(|rest| !rest.is_empty() && !rest.contains(char::is_whitespace))
            .map(str::to_owned)
    } else {
        None
    };
    if !release.as_deref().is_some_and(herdr_version_supported) {
        // An unsupported version is reported as unknown, matching the Lua
        // probe which only keeps the release once the floor check passes.
        return ProbeResult {
            path: exe.to_owned(),
            release: None,
            reason: Some("not a herdr X.Y.Z >= 0.9.3".to_owned()),
        };
    }
    let argv = [
        exe.to_owned(),
        "api".to_owned(),
        "schema".to_owned(),
        "--json".to_owned(),
    ];
    let schema = run(&argv, None);
    if schema.exit == Some(0) && schema_is_protocol_22(&schema.out) {
        return ProbeResult {
            path: exe.to_owned(),
            release,
            reason: None,
        };
    }
    ProbeResult {
        path: exe.to_owned(),
        release,
        reason: Some("bundled schema is not protocol 22".to_owned()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Selected {
    pub path: String,
    pub release: String,
}

fn rank(release: &str) -> [u64; 3] {
    let mut out = [0u64; 3];
    for (index, part) in release.split('.').take(3).enumerate() {
        out[index] = part.parse().unwrap_or(0);
    }
    out
}

/// Selected Herdr: the compatible candidate with the highest version; equal
/// versions keep PATH order. When none qualifies, every candidate's path and
/// refusal reason are reported.
pub fn select_herdr(
    candidates: &[String],
    run: &dyn Fn(&[String], Option<i64>) -> RunOut,
) -> Result<Selected> {
    let probes: Vec<ProbeResult> = candidates.iter().map(|exe| probe_herdr(run, exe)).collect();
    let mut best: Option<&ProbeResult> = None;
    for candidate in &probes {
        if candidate.reason.is_none() {
            let release = candidate.release.as_deref().unwrap_or("");
            let better = match best {
                None => true,
                Some(current) => rank(release) > rank(current.release.as_deref().unwrap_or("")),
            };
            if better {
                best = Some(candidate);
            }
        }
    }
    if let Some(best) = best {
        return Ok(Selected {
            path: best.path.clone(),
            release: best.release.clone().unwrap_or_default(),
        });
    }
    let rejected: Vec<String> = probes
        .iter()
        .map(|candidate| {
            format!(
                "{} ({}: {})",
                candidate.path,
                candidate.release.as_deref().unwrap_or("unknown"),
                candidate.reason.as_deref().unwrap_or("unknown")
            )
        })
        .collect();
    let detail = if rejected.is_empty() {
        "none".to_owned()
    } else {
        rejected.join("; ")
    };
    Err(Failure::new(format!(
        "{HERDR_GUIDANCE} Candidates on PATH: {detail}"
    )))
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

    fn run_with(out: &'static str) -> impl Fn(&[String], Option<i64>) -> RunOut {
        move |_, _| RunOut {
            exit: Some(0),
            out: out.to_owned(),
            err: String::new(),
            timeout: false,
        }
    }

    #[test]
    fn prompts_and_guidance_are_ported_verbatim() {
        assert!(STANDING_RULES.starts_with("You are the UML-viewer companion."));
        assert!(STANDING_RULES.contains("validResult exactly true"));
        assert!(STANDING_RULES.contains("at most three retries (1, 2, 4 seconds)"));
        assert!(STANDING_RULES.contains("Never\n   re-serialize the whole policy"));
        assert!(STANDING_RULES.ends_with("Do not commit or push unless asked.\n"));
        assert!(LAUNCH_PROMPT.starts_with("On launch: from this working directory,"));
        assert!(RECOVERY_PROMPT.starts_with("Recovery: do not scan sources or rewrite policy."));
        assert!(LEGACY_GUIDANCE.contains("Do not use --restart to migrate"));
        assert!(HERDR_GUIDANCE.contains("Herdr 0.9.3 or newer (protocol 22) is required."));
    }

    #[test]
    fn herdr_version_floor_accepts_only_dotted_numeric_at_or_above_093() {
        for version in ["0.9.3", "0.9.4", "0.10.0", "1.0.0", "10.0.0"] {
            assert!(herdr_version_supported(version), "{version}");
        }
        for version in [
            "0.9.2",
            "0.8.9",
            "abc",
            "0.9.4-rc.1",
            "v0.9.4",
            "0.9",
            "0.9.3.1",
            "",
        ] {
            assert!(!herdr_version_supported(version), "{version}");
        }
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

    #[test]
    fn candidates_follow_path_order_and_dedupe_canonical() {
        let probe = |path: &str| {
            if path.starts_with("/a/") || path.starts_with("/b/") {
                Some(Probe {
                    executable: true,
                    canonical: path.to_owned(),
                })
            } else {
                None
            }
        };
        assert_eq!(
            vec!["/b/herdr".to_owned(), "/a/herdr".to_owned()],
            herdr_candidates("/b:/none::/a:/b", &probe)
        );
        assert!(herdr_candidates("/a:/b", &(|_| None)).is_empty());
    }

    fn installed<'a>(
        releases: &'static [(&'static str, &'static str, u32)],
        calls: &'a std::cell::RefCell<Vec<Vec<String>>>,
    ) -> impl Fn(&[String], Option<i64>) -> RunOut + 'a {
        move |argv, _| {
            calls.borrow_mut().push(argv.to_vec());
            let entry = releases.iter().find(|(path, _, _)| *path == argv[0]);
            match entry {
                None => RunOut {
                    exit: Some(1),
                    out: String::new(),
                    err: "not found".to_owned(),
                    timeout: false,
                },
                Some((_, release, protocol)) => {
                    if argv[1] == "--version" {
                        RunOut {
                            exit: Some(0),
                            out: format!("herdr {release}"),
                            err: String::new(),
                            timeout: false,
                        }
                    } else {
                        RunOut {
                            exit: Some(0),
                            out: format!(
                                "{{\"$schema\":\"fixture\",\"protocol\":{protocol},\"schema_version\":1}}"
                            ),
                            err: String::new(),
                            timeout: false,
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn selects_the_highest_compatible_release_regardless_of_path_order() {
        let calls = std::cell::RefCell::new(Vec::new());
        let run = installed(
            &[
                ("/a/herdr", "0.9.3", 22),
                ("/b/herdr", "0.10.0", 22),
                ("/c/herdr", "0.9.4", 22),
            ],
            &calls,
        );
        let selected = select_herdr(
            &["/a/herdr".into(), "/b/herdr".into(), "/c/herdr".into()],
            &run,
        )
        .unwrap();
        assert_eq!("/b/herdr", selected.path);
        assert_eq!("0.10.0", selected.release);
    }

    #[test]
    fn falls_back_to_the_highest_protocol_22_release() {
        let calls = std::cell::RefCell::new(Vec::new());
        let run = installed(
            &[
                ("/a/herdr", "0.9.3", 22),
                ("/b/herdr", "0.10.0", 23),
                ("/c/herdr", "0.9.4", 22),
            ],
            &calls,
        );
        let selected = select_herdr(
            &["/a/herdr".into(), "/b/herdr".into(), "/c/herdr".into()],
            &run,
        )
        .unwrap();
        assert_eq!("/c/herdr", selected.path);
        assert!(!calls.borrow().is_empty());
    }

    #[test]
    fn keeps_path_order_between_equal_releases() {
        let calls = std::cell::RefCell::new(Vec::new());
        let run = installed(
            &[("/a/herdr", "0.9.4", 22), ("/b/herdr", "0.9.4", 22)],
            &calls,
        );
        let selected = select_herdr(&["/a/herdr".into(), "/b/herdr".into()], &run).unwrap();
        assert_eq!("/a/herdr", selected.path);
    }

    #[test]
    fn reports_every_rejected_candidate_when_none_is_compatible() {
        let calls = std::cell::RefCell::new(Vec::new());
        let run = installed(
            &[("/a/herdr", "0.9.2", 22), ("/b/herdr", "0.10.0", 23)],
            &calls,
        );
        let error = select_herdr(
            &[
                "/a/herdr".into(),
                "/b/herdr".into(),
                "/missing/herdr".into(),
            ],
            &run,
        )
        .unwrap_err();
        assert!(
            error
                .message
                .contains("/a/herdr (unknown: not a herdr X.Y.Z >= 0.9.3)")
        );
        assert!(
            error
                .message
                .contains("/b/herdr (0.10.0: bundled schema is not protocol 22)")
        );
        assert!(
            error
                .message
                .contains("/missing/herdr (unknown: not a herdr X.Y.Z >= 0.9.3)")
        );
        let empty = select_herdr(&[], &run).unwrap_err();
        assert!(empty.message.contains("Candidates on PATH: none"));
    }

    #[test]
    fn probes_version_before_the_bundled_schema_without_creating_resources() {
        let calls = std::cell::RefCell::new(Vec::new());
        let run = |argv: &[String], _: Option<i64>| {
            calls.borrow_mut().push(argv.to_vec());
            RunOut {
                exit: Some(0),
                out: if argv[1] == "--version" {
                    "herdr 0.9.3".to_owned()
                } else {
                    "{\"$schema\":\"fixture\",\"protocol\":22,\"schema_version\":1}".to_owned()
                },
                err: String::new(),
                timeout: false,
            }
        };
        let selected = select_herdr(&["/x/herdr".into()], &run).unwrap();
        assert_eq!("0.9.3", selected.release);
        assert_eq!(
            vec![
                vec!["/x/herdr".to_owned(), "--version".to_owned()],
                vec![
                    "/x/herdr".to_owned(),
                    "api".to_owned(),
                    "schema".to_owned(),
                    "--json".to_owned()
                ],
            ],
            calls.into_inner()
        );
    }

    #[test]
    fn rejects_missing_malformed_or_failed_schemas_without_further_commands() {
        let schemas: Vec<RunOut> = vec![
            RunOut {
                exit: None,
                out: String::new(),
                err: String::new(),
                timeout: false,
            },
            RunOut {
                exit: Some(1),
                out: "{\"$schema\":\"fixture\",\"protocol\":22,".to_owned(),
                err: String::new(),
                timeout: false,
            },
            RunOut {
                exit: Some(0),
                out: "not json".to_owned(),
                err: String::new(),
                timeout: false,
            },
            RunOut {
                exit: Some(0),
                out: "{\"$schema\":\"fixture\",\"protocol\":21,\"schema_version\":1}".to_owned(),
                err: String::new(),
                timeout: false,
            },
        ];
        for schema in schemas {
            let calls = std::cell::RefCell::new(Vec::new());
            let run = |argv: &[String], _: Option<i64>| {
                calls.borrow_mut().push(argv.to_vec());
                if argv[1] == "--version" {
                    RunOut {
                        exit: Some(0),
                        out: "herdr 0.9.3".to_owned(),
                        err: String::new(),
                        timeout: false,
                    }
                } else {
                    schema.clone()
                }
            };
            assert!(select_herdr(&["/x/herdr".into()], &run).is_err());
            assert_eq!(2, calls.borrow().len());
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

    #[test]
    fn schema_pattern_requires_protocol_22_after_the_schema_key() {
        assert!(schema_is_protocol_22(
            "{\"$schema\":\"https://x\",\"protocol\":22,\"schema_version\":1}"
        ));
        assert!(schema_is_protocol_22(
            "  { \"$schema\" : \"x\" , \"protocol\" : 22 , }"
        ));
        assert!(!schema_is_protocol_22(
            "{\"$schema\":\"x\",\"protocol\":21,\"v\":1}"
        ));
        assert!(!schema_is_protocol_22(
            "{\"protocol\":22,\"$schema\":\"x\"}"
        ));
        assert!(!schema_is_protocol_22(
            "{\"$schema\":\"x\",\"protocol\":22}"
        ));
        assert!(!schema_is_protocol_22("not json"));
    }

    #[test]
    fn probe_passes_literal_executable_paths() {
        let run = run_with("herdr 0.9.5");
        let probe = probe_herdr(&run, "/opt/herdr 0.9/herdr");
        // Version output matches but the schema probe fails: the stub always
        // answers the version text, so the schema is not protocol 22.
        assert_eq!(
            Some("0.9.5"),
            probe.release.as_deref().map(str::to_owned).as_deref()
        );
        assert!(probe.reason.is_some());
    }
}
