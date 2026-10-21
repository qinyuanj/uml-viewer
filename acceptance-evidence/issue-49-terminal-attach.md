# Issue #49 Terminal attach runtime evidence

> **Historical (JVM era).** This macOS record is not evidence for the current implementation; macOS is not committed (ADR 0007). Current evidence: [106-wsl](106-wsl/README.md).

Recorded 2026-10-16, macOS 27.0.1 / Darwin 27.0.0 arm64. Source fixes remain uncommitted. No real viewer was started. Quality tools are run separately by the coordinator; this investigation did not run CRAP, mutation, or IR. Commands below use `<repo>` and `<probe-root>` instead of personal filesystem paths. No credentials are included.

## Root causes and fixes

1. `sdef /System/Applications/Utilities/Terminal.app` defines `title displays settings name` on a settings set, not a tab. In a newly created, owned Terminal tab, reading this property returned -1728 and setting it to false returned -10006. Other custom/device/shell title settings succeeded in the same script. The unlocked runtime therefore reproduces the historical error without requiring a lock-screen or permission explanation. Removed this unsupported tab property assignment from `sketch/osascript`.
2. `do script` returns a tab belonging to a new window while `front window` can still identify an older window. Native probe returned new-tab window 19854, TTY `/dev/ttys001`, but front-window ID 19847. Replaced `id of front window` with `id of (first window whose selected tab is agentTab)`.

The property probe used these operations on its own tab only:

```applescript
set probeTab to do script "printf '\\nUML_ATTACH_PROBE_49_20261016\\n'"
get title displays settings name of probeTab
set title displays settings name of probeTab to false
set title displays device name of probeTab to false
set title displays shell path of probeTab to false
set title displays custom title of probeTab to true
```

Results: property read -1728; unsupported write -10006; supported writes success. No inference that all Terminal failures are caused by permissions is warranted.

## Targeted specs

```sh
clj -M:spec spec/uml_viewer/adapters/sketch_spec.clj
```

The added supported-property assertion was run before the source fix: 65 examples, 1 failure, 265 assertions. It expected `["custom title" "device name" "shell path"]` and observed the extra `"settings name"`. After removing that assignment: 65 examples, 0 failures. After adding the window-identity guard and implementing the window selector: 66 examples, 0 failures, 267 assertions. The window selector also has independent native before/after evidence, rather than relying on the source-string assertion alone.

## Public function runtime check

Each probe used a unique named Herdr server, never the default or another project's session:

```sh
herdr --session <probe-session> server
herdr --session <probe-session> status server
```

The server process was started from a fresh temporary directory. PID/start/cwd were checked with `ps -p <pid> -o pid=,lstart=,comm=` and `lsof -a -p <pid> -d cwd -Fn` before normal cleanup.

Clojure invocation used existing dependencies without starting a viewer:

```sh
clj -Sdeps '{:aliases {:native49 {:main-opts ["-e" "(require (quote [uml-viewer.adapters.sketch :as sketch]) (quote [uml-viewer.adapters.companion :as companion])) (with-redefs [companion/herdr (fn [] \"/opt/homebrew/bin/herdr\")] (prn (sketch/open-herdr-window! \"<probe-session>\"))) (shutdown-agents)"]}}}' -M:spec:native49
```

Herdr version was 0.9.5, protocol 22. This establishes targeted runtime behavior for that version, not full project acceptance of 0.9.5.

Final identity probe: session `uml49-d37e2a58`, owned server PID 94148. Public function returned exit 0, `:terminal :terminal`, `:window-id "19855"`, no -10006. Reading Terminal window 19855 returned TTY `/dev/ttys001`, processes containing `herdr`, and a title containing exactly `session attach uml49-d37e2a58`. Process inspection found one client for that unique session. Server stop returned success, actual server exit was 0, session delete returned success. Window 19855 was closed after its process list became empty.

Two preliminary invocations had harness mistakes: `-M:spec -e` routed `-e` to Speclj and exited 255; a standalone dependency-loading invocation exceeded its 30-second harness timeout. Neither is product failure evidence. Their named servers were normally stopped (actual exit 0) and deleted. The corrected alias override above is the runtime evidence of record.

## Focus, input, Unicode, detach probe

Session `uml49-a77a3e62`, owned server PID 94908; fresh workspace created using:

```sh
herdr --session uml49-a77a3e62 workspace create --cwd <probe-root> --no-focus
```

Returned workspace `w1`, pane `w1:p1`, terminal `term_65dee9b068d141`; shell PID later observed as 94933. Public attach returned Terminal window 19856. By-ID read confirmed TTY `/dev/ttys003`, running attach process, and the exact session in the window title.

Explicit AX probe (without swallowing errors):

```applescript
tell application "System Events"
  tell process "Terminal"
    set w to first window whose name contains "uml49-a77a3e62"
    perform action "AXRaise" of w
    return "raised=" & name of w & " frontmost=" & frontmost
  end tell
end tell
```

Exit 0, `raised=<owned-session-window-title> frontmost=false`. Therefore AXRaise action acceptance is verified; application keyboard focus from the public path is NOT VERIFIED. The production script catches AXRaise errors, so its successful return alone cannot establish focus.

After explicitly setting Terminal frontmost and raising the unique owned session window, this synthetic input was sent:

```applescript
keystroke "printf 'UML49_INPUT_中文_OK\n'"
key code 36
```

Then:

```sh
herdr --session uml49-a77a3e62 pane read w1:p1 --source recent-unwrapped --lines 15
```

Observed text contained `UML49_INPUT_aa_OK` with transformed quotes; zsh reported command not found and entered a quote continuation prompt. Thus input reached the owned pane and is readable, but the synthetic-keystroke route did NOT faithfully transmit Chinese or punctuation. This is not evidence of a Herdr Chinese rendering defect, and it is not a passing Chinese input/echo acceptance result. No pasteboard content was read or overwritten.

Sent `Ctrl+B q` using:

```applescript
tell application "System Events" to tell process "Terminal"
  keystroke "b" using control down
  keystroke "q"
end tell
```

AppleScript exit 0. Afterwards `pane process-info --pane w1:p1` reported the same shell PID 94933, and `status server` still reported running / protocol 22. However the attach client's exit was not measured before server shutdown, so the detach result is only PARTIAL: pane/server survival is measured, successful client detach is NOT VERIFIED.

Cleanup: verified server PID/start/cwd, `herdr session stop uml49-a77a3e62 --json` exit 0, awaited actual server exit 0, then `herdr session delete uml49-a77a3e62 --json` exit 0. Window 19856 had an empty process list and was closed by its recorded ID. Earlier owned probe windows were also closed after their shells exited normally. No other Terminal windows or sessions were closed.

## Final focus fix and issue-scope acceptance

The earlier AXRaise-only result above is superseded by this fix and runtime check. Setting AXMain/AXFocused on the owned AX window failed with -10006. Setting the specifically identified Terminal `agentWindow` index to 1 and then the Terminal process `frontmost` to true succeeded. The final public script uses:

```applescript
set agentWindow to first window whose selected tab is agentTab
set winID to id of agentWindow
set index of agentWindow to 1
end tell
tell application "System Events"
  tell process "Terminal"
    set frontmost to true
  end tell
end tell
return winID
```

It does not call `activate`, does not AXRaise all windows, and does not subsequently guess the target through a front-window AXRaise. The old swallowed AXRaise block is removed. No global Terminal settings or shell preferences were changed.

A copied isolated fixture with the two focus statements removed ran the new regression assertions: exit 1, 66 examples, 1 failure, 269 assertions. The final source ran `clj -M:spec spec/uml_viewer/adapters/sketch_spec.clj`: 66 examples, 0 failures, 271 assertions. The shared worktree was never reverted for this comparison.

Final native session `uml49-18d4eb63`, server PID 97026:

- Baseline Terminal window IDs: 19865, 19863, 19850, 19847.
- Public `open-herdr-window!` exit 0, `:terminal :terminal`, `:window-id "19867"`.
- Immediate observation: Terminal front-window ID = owned ID = 19867, TTY `/dev/ttys001`, System Events Terminal process `frontmost=true`.
- One second later: same front-window ID and `frontmost=true`.
- Both reads showed a title containing the exact `herdr session attach uml49-18d4eb63` command, providing an accurate unique-session label. The issue does not require an immutable custom project title; shell/Herdr OSC changes do not invalidate this measured session label.
- During the check all four baseline windows remained present in the same relative order. Only the newly owned target was put first.
- PID/start/cwd checked before cleanup; `session stop` succeeded, actual server exit 0, `session delete` succeeded, then exited owned window 19867 closed. Window inventory returned exactly to the baseline.
- The separately owned, exited AX/title probe windows 19863 and 19865 were then normally closed; unrelated windows were not closed.

**Issue #49's opening, focus, and session-label behavior now has direct automated native runtime evidence.** Degradation assessment: the historical -10006 was caused by an invalid tab property; current failure handling retains the companion and prints a manual attach command, so it continues to provide a usable fallback. No extra warning or global preference change was added. The source fixes are local/uncommitted; this investigator did not comment on or close the issue.

## Additional contract boundaries

These are distinct from #49's opening/focus/label scope:

- Faithful Chinese input/echo and human visual rendering remain unverified; synthetic keystroke corruption prevented a valid check.
- Detach client exit remains unverified; pane/server survival after the key sequence was measured.
- No screenshot or human visual observation was collected.
- The host was not deliberately relocked; lock-screen behavior was not re-run.
- These targeted native checks do not establish full lifecycle acceptance of Herdr 0.9.5.

## Issue #48 real shutdown-hook verification

A separate JVM exercised real production creation and cleanup with a harmless `/bin/sleep 120` child, without launching Claude or a viewer. A fresh temporary project root and unique session `uml-viewer-92eff689-ce249872` were used. No ownership was fabricated: `companion/create!` itself created and persisted the complete server/pane/runner/supervisor identity.

The JVM expression, supplied using the same `-Sdeps` alias technique as above, was:

```clojure
(require '[uml-viewer.adapters.herdr :as herdr]
         '[uml-viewer.adapters.companion :as companion]
         '[uml-viewer.adapters.sketch :as sketch])
(companion/select-herdr!)
(let [owned (companion/create! (herdr/herdr nil)
                              "uml-viewer-92eff689-ce249872"
                              "<fresh-probe-root>"
                              ["/bin/sleep" "120"])]
  (println :created (:session owned)
           :server (:server-pid owned) :shell (:shell-pid owned))
  (reset! sketch/!project-root "<fresh-probe-root>")
  (#'sketch/install-shutdown-hook!)
  (println :exiting-through-real-hook)
  (System/exit 0))
```

Results: independent JVM exit 0; server PID 95733, shell PID 95760. After JVM exit, `companion.edn` was `{}` and `recovery.edn` was `:phase :cancelled`, `:reason "Viewer closed; recovery revoked"`, attempts 0. The session was absent from `herdr session list --json`. `ps -p 95733,95760,95858,95867 -o pid=,ppid=,comm=` returned no rows (server, shell, sleep child, recovery controller all gone). This verifies real `install-shutdown-hook! -> shutdown-children! -> owned-record! -> backend cleanup!` including recovery cancellation, not merely a stub.

Two setup invocations failed before creating any resources: zero-argument `(herdr/herdr)` raised ArityException; an `uml48-...` session name was rejected by the production isolated-session-name guard. The successful run used `(herdr/herdr nil)` and the required `uml-viewer-...` name. Those failures did not bypass any guard or leave resources.

This proves normal JVM shutdown cleanup of a genuinely owned supervised child. It does not constitute an actual menu/Dock Quit interaction or restart-preservation check; the coordinator's separate hook/restart unit evidence remains separate.

## Mutation worker orphan identity gap

Read-only investigation identified the mutation-created session `uml-viewer-7ea9585c-73505ec8`, not a probe created by this investigation:

- Herdr server PID 91457; start `2026-10-16 13:35:40` local.
- Pane `w1:p1`, shell PID 91486; same start second.
- Runner JVM PID 91585; start `13:35:42`; parent 91486.
- Claude child PID 91605; start `13:35:43`; parent 91585.
- Server/runner/child cwd: `<repo>/target/mutation-workers/run-6846dde6-9f4b-4e9e-a11f-10d55b21e7bf/worker-3/root`.
- Runner argv points to `.uml-viewer/uml-viewer-7ea9585c-73505ec8-runner.edn.launch.edn` under that worker root.
- Explicit-session status: Herdr 0.9.5, protocol 22; explicit `pane process-info --pane w1:p1` confirmed the runner/Claude PIDs and worker cwd.

After mutation completion the worker root's companion ownership and launch manifest were absent. These process observations establish origin but do not restore the complete durable ownership/supervisor proof required by production cleanup. No replacement record was written, no missing-ownership refusal was bypassed, and no orphan process/session was stopped by this investigator. The coordinator was notified of this retained residual resource.

### Explicitly authorized orphan release (2026-10-16)

The subsequent cleanup was authorized only for session `uml-viewer-7ea9585c-73505ec8`, contingent on matching existing process identities and no live recovery controller. AGENTS.md, GLOSSARY.md, the orphan evidence above, domain instructions, and relevant Herdr ADRs were read first. No viewer, quality tool, or production cleanup retry was run.

Read-only checks at approximately 13:49 local (+08:00):

- Explicit-session `status server` reported running, Herdr 0.9.5 / protocol 22. `api snapshot` reported workspace `w1`, pane `w1:p1`, terminal `term_65dee88e406f71`, idle Claude, and the same worker cwd as above. `pane process-info --pane w1:p1` reported shell 91486, runner 91585, and Claude 91605, with the expected runner launch path and worker cwd.
- `ps` confirmed server 91457 / parent 1 / start 13:35:40, shell 91486 / parent 91457 / start 13:35:40, runner 91585 / parent 91486 / start 13:35:42, and Claude 91605 / parent 91585 / start 13:35:43 on 2026-10-16. `lsof -a -p 91457,91486,91585,91605 -d cwd -Fn` confirmed the exact worker cwd for all four. The attached client 91766 / parent 91721 / start 13:35:47 had argv `herdr session attach uml-viewer-7ea9585c-73505ec8`; its login shell 91721 started at 13:35:46, and both had the user's home cwd. That Terminal shell/window was left alone.
- The worker path was no longer a source tree, but did exist as a residual directory containing only `.uml-viewer/`; that directory contained `recovery.edn` and `recovery-gate.lock`. Thus the earlier description of a deleted cwd needs this qualification. Companion ownership, recovery launch manifest, recovery control record, and runner launch manifest were absent. Nothing was recreated.
- The existing `recovery.edn`, mtime 13:35:55.208731 +08:00, contained exactly `{:phase :failed, :reason "Recovery controller identity is missing or changed"}`. A full process-table check found no `uml-viewer.adapters.recovery controller` process. Enumerating every Java executable found only runner 91585, in `runner` mode. Recovery was therefore not active: the retained journal was terminal `:failed` and no controller existed. No cancellation or replacement identity was invented.
- The pre-release session inventory contained only stopped `default` and this running named session. The matching PID/start/cwd/argv and Herdr pane observations established that the target was the mutation test resource identified above.

With those conditions met, the official explicit-session commands were executed once each, in sequence:

```sh
/opt/homebrew/bin/herdr session stop uml-viewer-7ea9585c-73505ec8 --json
# Wait for ps to show all verified session processes absent before delete.
/opt/homebrew/bin/herdr session delete uml-viewer-7ea9585c-73505ec8 --json
```

`session stop` exited 0 and returned `"stopped":true`, `"running":false`. A bounded read-only process wait confirmed that server 91457, shell 91486, runner 91585, Claude 91605, and attach client 91766 were all absent by 13:49:59.360716 +08:00. The intermediate session list showed the named session stopped. Only then was `session delete` called; it exited 0 and returned `"deleted":true`.

Final verification at 13:50:08.606135 +08:00: `ps` for those five PIDs returned exit 1 with no rows; no Java processes remained; `herdr session list --json` contained only the original stopped `default`; the target Herdr session directory no longer existed. These checks measure actual process disappearance, not the old server/child exit codes, which were not available to this non-parent observer.

This was the user's authorized explicit Herdr session release, not a retry of production cleanup after missing ownership. No signal/kill command, ownership fabrication, global configuration change, other-session cleanup, Terminal window close, source edit, quality run, commit, push, or external comment was performed. Only this evidence document was updated; residual worker files were retained.
