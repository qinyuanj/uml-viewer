# macOS Herdr native acceptance — spec #32

> **Historical (JVM era).** This macOS record is not evidence for the current implementation; macOS is not committed (ADR 0007). Current evidence: [106-wsl](106-wsl/README.md).

Status: **final — PASS** (consolidated by ticket #44, 2026-10-15). All
executable criteria passed: #33, #34 (after three companion fixes landed
during acceptance: 0b6f1ea sun_path, 6355aa4 login shells, 5e4407e
canonical-mode retry; initial runs FAIL → defect #45, fixed and closed),
#35, #36, #37, #38, #39, #40 (PASS with one timing observation), #41, #42,
#43. Terminal attach is **NOT VERIFIED** (manual criterion; handed to
follow-up issue **#49**, superseding #46). Follow-up observations live in
the 问题清单 section (#47, #48, and smaller items). This document is the
single acceptance log required by spec #32 user story 17.

Maintainer authorizations (given in the orchestrating Claude session,
recorded here for traceability):
- 2026-10-14 「解决阻断」: after #34's FAIL blocked the remaining criteria,
  the maintainer authorized diagnosing **and fixing** the blocking defects
  during acceptance. This deviates from spec #32's "不改动被测代码" default;
  the three fix commits (0b6f1ea, 6355aa4, 5e4407e) land under that
  authorization, each with red-green specs, and every fix was verified by a
  real cold-start re-run before #34 was re-judged PASS.
- 2026-10-15 「授权你处理」: the maintainer authorized the closing
  commits — the `./uml` mode fix (c2b3c5c, spec said the maintainer decides
  commit-or-revert of the mode change), the evidence import (3ef8a99,
  0badde2) and the three documentation updates (d0276e5). No push was
  performed or authorized.

Evidence root: `acceptance-evidence/` in this worktree
(`/Users/billyq/Dev/unclebob/uml-acc-32/project-a/`). No
credentials or API keys appear in any captured artifact. The host screen
was locked for the entire 2026-10-15 session (`screenIsLocked=1`), so no
screenshots were taken: window evidence is CGWindowList enumeration, and
GUI-only interactions used production substitute paths, each noted at its
criterion (red-button close → `shutdown-children!`; inspector proposal
click → `write-session!`; window chrome events were never synthesized).

## Environment (recorded 2026-10-14, automated evidence unless noted)

| Item | Value | Evidence |
|---|---|---|
| Checkout (acceptance execution) | `5e4407e` (`fix(companion): retry Herdr pane run while a fresh pty is in canonical mode`) — projects A/B and both runtime clones synced here at #37; acceptance started at c714f83 and fixes 0b6f1ea/6355aa4/5e4407e landed during #34 | `git rev-parse HEAD`, `37/` |
| Branch head (consolidation) | `c2b3c5c` (`chore(wrapper): make ./uml directly executable`, branch `spec-32/macos-herdr-acceptance`) — adds only the `./uml` mode fix already applied locally during acceptance | `git log -1` |
| Platform | macOS 27.0.1 (26A434), Darwin 27.0.0, arm64, native (no VM) | `sw_vers`, `uname -m`, `uname -sr` |
| Herdr | 0.9.4 at `/opt/homebrew/bin/herdr` (unique on PATH), socket protocol 22 verified (preflight schema probe on every launch) | `which herdr`, `herdr --version`, launch logs |
| Screen state | locked for the whole 2026-10-15 session (`CGSSessionScreenIsLocked=1`); host also slept ~6 h mid-#39 (02:47→08:37, no effect) | `41/screen-lock.txt`, `39/` notes |
| Clojure CLI | 1.12.6.1673 (`clojure` and `clj` identical) | `clojure -version`, `clj --version` |
| JDK | Temurin 25 LTS (`openjdk version "25" 2026-08-18 LTS`, build 25+1-LTS) | `java -version` |
| PowerShell | pwsh 7.6.7 | `pwsh --version` |
| Claude Code | 2.1.299 at `/opt/homebrew/bin/claude` (real CLI) | `which claude`, `claude --version` |

Project A (main acceptance):
`/Users/billyq/Dev/unclebob/uml-acc-32/project-a` — detached
worktree at 5e4407e (synced from c714f83 at #37). Project B (parallel
control): `/Users/billyq/Dev/unclebob/uml-acc-32/project-b` —
same checkout. Both are independent project roots with live `.uml-viewer/`
state and healthy companions at consolidation time (A: session
`uml-viewer-5f26e88b-bcc99ab2`; B: `uml-viewer-7db6871f-aa6c2aa8`). The
`./uml` chmod 755 mode fix applied locally during acceptance is now
committed at branch head c2b3c5c.

Main-repo pre-acceptance snapshot (for #42 legacy-refusal diff):
`acceptance-evidence/main-repo-uml-viewer-snapshot/` (primary) and
`/Users/billyq/Dev/unclebob/uml-acc-32/main-repo-snapshot/`
(redundant copy). SHA-256 verified identical to the live
`.uml-viewer/` files at snapshot time:

| File | SHA-256 |
|---|---|
| companion.edn | 488351c1c12bc1168489cacf877bfc14731fff6cecf22b6438a33c865053b986 |
| session.edn | 2195250b44df29d5a137fdd3cf35a829fa15a62aa28b286431f7baa7c1e3b877 |
| to-agent.edn | e6481c383d03c23d2669c59e17f22ed23dfaa88ef63b99acd497573d0d8fdded |
| to-viewer.edn | b36fb3bcefb9265e3c0aa0fcb9973bfd4bac6a45e62b2d8e7d1a57aca34ceae4 |

Pre-snapshot state (manual observation of file contents): companion.edn is
a legacy tmux record (`:session "uml-viewer-uml-viewer-a3192be9"`,
`:pane "%0"`, no `:backend` field); to-agent.edn holds one queued
`:context` mail (id 4); to-viewer.edn holds one queued `:display` mail
(id 6); session.edn is a zeroed view state. No companion/viewer processes
running.

## GUI permission precheck (2026-10-14, automated probes; no real window touched)

| Capability | Probe | Result |
|---|---|---|
| Accessibility (System Events) | `osascript -e 'tell application "System Events" to get name of every process'` | exit 0, full process list returned — accessibility granted to the terminal host |
| Window/app enumeration | `lsappinfo list` | exit 0, 121 entries |
| Screen recording | `screencapture -x /tmp/uml-acc-32-screencap-test.png` (full-screen probe, deleted immediately, not retained) | exit 0, 3456x2234 PNG produced. Caveat: macOS `screencapture` can exit 0 with a wallpaper-only image when permission is missing; per-window content correctness is verified for real at #40/#43 evidence capture (`screencapture -l <windowid>`) |
| Keystroke send (⌘Q/⌘W via System Events) | not probed — would require operating a real window, out of precheck scope | inferred available from granted accessibility; actual send deferred to #41/#43/#44 execution |

No permission-insufficient item found in precheck; nothing pre-marked NOT
VERIFIED from this section.

## Timeline

| Time (local) | Event | Evidence |
|---|---|---|
| 2026-10-14 | Ticket #33 skeleton: worktrees A/B created detached at c714f83; `./uml` chmod 755 in both; snapshots taken; environment and GUI precheck recorded above | this section; `main-repo-uml-viewer-snapshot/` |
| 2026-10-14 22:32:53 +0800 | #34 pre-launch baseline: `project-a/.uml-viewer/` absent (no state, no installed runtime); `herdr session list` = only `default` (stopped), `herdr status` server not running; `pgrep -fl java` none, `pgrep -fl herdr` none; `pgrep -fl pwsh` = one foreign process pid 27646 running the **main repo** `uml.ps1` (not project A, left untouched) | `herdr session list`, `herdr status`, `pgrep` output this section |
| 2026-10-14 22:33:16 +0800 | #34 runtime install (documented setup per AGENTS.md): `scripts/get-uml-viewer --install-only` in project A, exit 0; cloned uml-viewer @ c714f83 into gitignored `.uml-viewer/uml-viewer/`; `git status` unchanged afterwards (only pre-existing ` M uml` mode fix + untracked `acceptance-evidence/`); no tracked file modified | installer stdout; `git -C .uml-viewer/uml-viewer rev-parse HEAD` = c714f83e25de4849c0757d8f27d26addf3c0b181; `stat` birth time |
| 2026-10-14 | Ticket #42 executed: `./uml` in the main repo refused the legacy tmux companion.edn at ownership verification (exit 1, `legacy-guidance` on stderr); post-run SHA-256 diff of `.uml-viewer/` against the snapshot is zero; no JVM/Herdr side effects | #42 section below |
| 2026-10-14 22:36:48 +0800 | #34 cold start: `./uml` in project A, exit 0 after 2 s (`uml: selected Herdr 0.9.4 at /opt/homebrew/Cellar/herdr/0.9.4/bin/herdr`; launcher pid 28673) | `34-uml-invocation.txt` |
| 2026-10-14 22:36:52 +0800 | #34 failure: viewer JVM `start!` wrote provisional companion.edn, launched Herdr server pid 28696 (session `uml-viewer-5f26e88b-7cabe77b-4ddb-4d0b-8194-481212cfec2f`), passed status/compatibility, then `workspace create` failed — server answered `{"error":{"code":"server_unavailable","message":"request handling failed: receiving on a closed channel"}}` and the server process exited ~2 ms later (`app.shutdown outcome=completed` in the session server log). Viewer JVM died (exit 1, stack trace in viewer log); launcher 28673 and server 28696 both gone; session left `stopped`; companion.edn stuck `:provisional true` | `34-failure/uml-viewer-log.txt`, `34-failure/herdr-server.log`, `34-failure/companion.edn`, `34-coldstart-watch.log` |
| 2026-10-14 22:37:42–22:40:58 +0800 | #34 `:display` watch: 195 s poll of `.uml-viewer/`, `herdr session list`, `pgrep java` — no `:display` mail, no viewer window, no java/herdr processes; timed out | `34-coldstart-watch.log` |
| 2026-10-14 22:44 +0800 | #34 defect filed as issue #45 (bug, needs-triage); project-A residue cleaned after evidence copy (provisional companion.edn / viewer-process.json / uml-viewer-log.txt removed; stopped session `uml-viewer-5f26e88b-7cabe77b-...` deleted via `herdr session delete`, exit 0, inventory back to baseline `default` only) | #45; `herdr session list` |
| 2026-10-14 22:46 +0800 | #34 control setup: Herdr 0.9.3 official release binary (`herdr-macos-aarch64` from github.com/herdrdev/herdr v0.9.3) installed isolated at `uml-acc-32/herdr-0.9.3/herdr` (homebrew 0.9.4 untouched); `herdr --version` = 0.9.3, `api schema --json` protocol 22 | `34-failure/control-0.9.3/` setup notes |
| 2026-10-14 22:47:26 +0800 | #34 accidental second 0.9.4 run: PATH-prepending 0.9.3 did NOT select it — ADR 0004 picks the *highest* compatible release on PATH, so 0.9.4 (also on PATH) won; run failed identically (server pid 32564, session `...-31591941-...`). Doubles as 0.9.4 reproduction #2 (same server-log signature). Evidence kept, residue cleaned, session deleted | `34-failure/repro-2/`; `34-uml-invocation-0.9.3.txt` (first half) |
| 2026-10-14 22:49:18 +0800 | #34 0.9.3 control cold start: exclusive-PATH shim (`herdr-0.9.3-shim/`: herdr→0.9.3 binary + pwsh/clojure/clj/claude/git symlinks; PATH=shim+/usr/bin+/bin+/usr/sbin+/sbin); preflight printed `selected Herdr 0.9.3 at .../herdr-0.9.3/herdr`; wrapper exit 0 (launcher pid 33026) | `34-uml-invocation-0.9.3.txt` |
| 2026-10-14 22:49:21 +0800 | #34 0.9.3 control FAILED identically: server pid 33054 (session `uml-viewer-5f26e88b-3c0b70fa-...`) answered `workspace create` with `server_unavailable: receiving on a closed channel` and self-exited (same `update.check.start` adjacency in its server log); viewer JVM exit 1; `:display` never arrived in the 195 s watch. 3/3 identical failures across both versions | `34-failure/control-0.9.3/`, `34-coldstart-watch-0.9.3.log` |
| 2026-10-14 22:58 +0800 | #34 final cleanup (team-lead authorized): 0.9.3 control residue removed (provisional companion.edn, viewer-process.json, uml-viewer-log.txt), stopped session `...-3c0b70fa-...` deleted (exit 0). Project A back to clean baseline: `.uml-viewer/` holds only the installed runtime, `herdr session list` = `default` (stopped) only, no java/herdr processes, `git status` = pre-existing ` M uml` mode fix + untracked `acceptance-evidence/` | this row's verification output |
| 2026-10-14 | #44 consolidation (this file finalized): verdict FAIL recorded; #35–#41/#43 marked not executed (blocked by #45); problem list section added; terminal attach handed to follow-up issue #46; summary comments posted to #32 and #44 | this file; #32/#44 comments |
| 2026-10-15 00:11:43 +0800 | #34 re-run baseline (after #45 fix): project A worktree and runtime clone `.uml-viewer/uml-viewer` both at `0b6f1ea` (`fix(companion): shorten Herdr session names to fit macOS sun_path`); `herdr session list` = only `default` (stopped); `pgrep -fl java` none; `.uml-viewer/` holds only the installed runtime (`uml-viewer/`) | `git rev-parse HEAD` ×2, `herdr session list --json`, `pgrep`, `ls` |
| 2026-10-15 00:12:47 +0800 | #34 re-run cold start: `./uml` in project A, exit 0 (`uml: selected Herdr 0.9.4 at /opt/homebrew/Cellar/herdr/0.9.4/bin/herdr`; launcher pid 78941) | `34-rerun-uml-invocation.txt` |
| 2026-10-15 00:12:48 +0800 | #34 re-run: sun_path fix validated — session `uml-viewer-5f26e88b-8c637e27` (28 chars, new format; socket 76 B, client socket path 83 B, both < 104 B sun_path); Herdr server pid 78964 launched, status/compatibility passed, `workspace create` answered `outcome=ok` (the exact call that killed the server in all three 2026-10-14 runs); workspace w1 / pane w1:p1 / shell-pid 78990 recorded | `34-rerun/herdr-server.log`, `34-rerun/herdr-session.json`, `34-rerun/status-server.json` |
| 2026-10-15 00:12:48 +0800 | #34 re-run NEW failure (distinct from #45): viewer JVM died at companion provisioning step "inject runner command into pane" (herdr.clj:272) with `Unsupported Herdr pane shell; command was not sent; ownership retained {:shell "-zsh"}` — `process-owner!` reads `ps -o comm=`, which is `-zsh` for a macOS login shell, and `pane-command!`'s allowlist `#{sh bash zsh dash}` rejects the leading dash. JVM exit 1; `:display` never arrived; companion.edn retained at `:provisional true` per contract; Herdr server 78964 and pane shell 78990 left running (scene preserved, no cleanup) | `34-rerun/uml-viewer-log.txt`, `34-rerun/companion.edn.provisional`, `34-rerun/pane-process-info.json`, `34-rerun/ps-shell-comm.txt` |
| 2026-10-15 00:13:39–00:17:37 +0800 | #34 re-run `:display` watch: 48 polls × 5 s of `to-viewer.edn`, companion.edn and java processes — no `:display` mail, companion.edn stayed `:provisional true`, viewer JVM never reappeared; timed out. Post-watch scene check: session `uml-viewer-5f26e88b-8c637e27` still `running`, server pid 78964 and pane shell pid 78990 (`-zsh`) alive, left untouched for debugging; reported to team lead, issue kept open | `34-rerun-coldstart-watch.log`, `herdr session list --json`, `ps` |
| 2026-10-15 00:19:02 +0800 | #34 re-run 2 baseline (after login-shell fix `6355aa4`): worktree and runtime clone both at `6355aa4de02200ac182b049ff3d271c749852e18`; team lead cleaned the previous scene (session 8c637e27 stopped+deleted, provisional files removed); `herdr session list` = only `default` (stopped); `pgrep -fl java` none; `.uml-viewer/` = runtime + leftover `uml-viewer-5f26e88b-8c637e27-runner.edn.launch.edn` manifest from the previous run | `git rev-parse HEAD` ×2, `herdr session list --json`, `pgrep`, `ls` |
| 2026-10-15 00:19:24 +0800 | #34 re-run 2 cold start: `./uml` in project A, exit 0 (`uml: selected Herdr 0.9.4`; launcher pid 81458) | `34-rerun2-uml-invocation.txt` |
| 2026-10-15 00:19:31 +0800 | #34 re-run 2: login-shell fix validated — `pane-command!` accepted the `-zsh` login shell and built the runner command; session `uml-viewer-5f26e88b-999df526` (28 chars), server pid 81480, `workspace create` ok | `34-rerun2/herdr-server.log`, `34-rerun2/status-server.json` |
| 2026-10-15 00:19:31 +0800 | #34 re-run 2 NEW failure (third distinct defect): `pane run` rejected with `pane_send_failed: input contains a 3130-byte line but the terminal is in canonical mode with a 1024-byte limit; wait for the shell prompt and retry` — the single-line java `-cp` runner command (3130 B) exceeds the macOS PTY canonical-mode line limit while the freshly spawned pane has not reached a shell prompt; the adapter sends immediately after pane spawn with no shell-readiness wait. Server log: `pane.send_input outcome=error`; server stayed alive (`persist.save ok`). JVM exit 1; `:display` never arrived; companion.edn retained `:provisional true`; session/server/shell left running (scene preserved) | `34-rerun2/uml-viewer-log.txt`, `34-rerun2/companion.edn.provisional`, `34-rerun2/pane-process-info.json` |
| 2026-10-15 00:25:11 +0800 | #34 re-run 3 baseline (after canonical-mode retry fix `5e4407e`): worktree and runtime clone both at `5e4407e038a1bc618fd9a5f42c07a5b6e112cc40`; team lead cleaned the re-run 2 scene (session 999df526 stopped+deleted, provisional/runner manifests removed); `herdr session list` = only `default` (stopped); `pgrep -fl java` none; `.uml-viewer/` = runtime only | `git rev-parse HEAD` ×2, `herdr session list --json`, `pgrep`, `ls` |
| 2026-10-15 00:25:24 +0800 | #34 re-run 3 cold start: `./uml` in project A, exit 0 (`uml: selected Herdr 0.9.4`; launcher pid 83284) | `34-rerun3-uml-invocation.txt` |
| 2026-10-15 00:25:38–00:25:43 +0800 | #34 re-run 3: **companion create! completed end-to-end for the first time on macOS** — canonical-mode retry fix validated (the same 3130-B runner command was delivered once the fresh pty left canonical mode; no `pane_send_failed`). companion.edn finalized `:provisional false` (birth/mtime 00:25:43): session `uml-viewer-5f26e88b-6fc7dc58` (28 chars; client socket path 83 B < 104 B sun_path), server pid 83305, pane shell pid 83331, workspace w1, pane w1:p1, `:backend :herdr`; runner ack `:phase :started` (child pid 83410); recovery controller running (pid 83418, generation 0, attempts 0). Non-fatal deviation: Terminal tab-title osascript failed `-10006` (`title displays settings name`), companion retained with printed manual-attach guidance, `:window-id nil` | `34-rerun3/companion.edn.final`, `34-rerun3/companion-stat.txt`, `34-rerun3/recovery.edn`, `34-rerun3/status-server.json`, `34-rerun3/uml-viewer-log.txt` |
| 2026-10-15 00:26:26–00:26:36 +0800 | #34 re-run 3 `:display`: companion finished launch work in 59 s (policy updated from real namespaces, `./uml ir` exit 0, `examples/uml-viewer.edn` regenerated mtime 00:26:29), sent `:display` (mail id 1); the viewer consumed it — `to-viewer.edn` reads `{:next-id 2, :queue []}` (file birth 00:26:36). Viewer JVM pid 83289 alive, `Watching examples/library.edn` at startup then mtime-reloaded the regenerated diagram. Note: a live-file poll for the literal `:display` can never observe an already-popped mail — the first watch loop's `grep -c` idiom also false-fired once (documented in the watch log) | `34-rerun3/companion-pane-read.txt`, `34-rerun3/to-viewer.edn.consumed`, `34-rerun3-coldstart-watch.log` |
| 2026-10-15 00:28–00:30 +0800 | #34 re-run 3 GUI evidence: CGWindowList enumeration proves the viewer window on screen — id 18484, owner `main`, title `UML viewer`, 1470×930 @(129,107), layer 0; companion Terminal window id 18482 (`herdr session attach uml-viewer-5f26e88b-6fc7dc58`). **Screenshots NOT POSSIBLE from the acceptance agent host**: Screen Recording TCC is not granted to it — `screencapture -l <windowid>` fails `could not create image from window` (exit 1) for both windows and a full-screen probe is fully black. This corrects the 2026-10-14 precheck's optimistic screen-recording read (its own wallpaper-only caveat applies). Window-presence evidence is the enumeration record | `34-rerun3/window-enumeration.txt` |
| 2026-10-15 00:30 +0800 | #34 re-run 3 uniqueness check: herdr inventory = exactly one `uml-viewer-*` session (`uml-viewer-5f26e88b-6fc7dc58`, running) beside `default` (stopped); exactly one wrapper launch (`34-rerun3-uml-invocation.txt`, launcher pid 83284); companion.edn written by one create! flow; recovery.edn generation 0 / attempts 0. Pane foreground processes are exactly the companion `claude --session-id b7e464ee-…` (pid 83410) and its runner java (pid 83395) — one companion, no duplicates | `34-rerun3/session-list.json`, `34-rerun3/api-snapshot.json`, live `pane process-info` |
| 2026-10-15 00:41:51 +0800 | #36 to-viewer FIFO: 3 `:display` mails enqueued atomically (ids 2/3/4 = library, uml-viewer, library); viewer popped head-first with per-pop rewrites: `[2,3,4]`→`[3,4]`→`[4]`→`[]` (8/92/110 ms apart); restore mail id 5 consumed in 220 ms at 00:42:18, acceptance diagram back on screen | `36/to-viewer-fifo-samples.log`, `36/to-viewer.after-phase1.edn` |
| 2026-10-15 00:43:00–00:44:51 +0800 | #36 to-agent FIFO: 3 distinct `:context` mails enqueued via production `write-command!` (ids 1/2/3 = real → proposal "test" → real); production `notify-agent!` wake (herdr `agent prompt` + token + 10 s ack); companion took ~21 s, ack window expired (`mail remains queued`, returned false — incidental retention observation), then popped all three and reported in-pane 「已按序处理…real → proposal "test" → real」; queue drained | `36/to-agent-fifo-samples.log`, `36/wake-phase2-output.txt`, `36/companion-pane-final.txt` |
| 2026-10-15 00:49:27–00:49:48 +0800 | #36 failed-wake retention: companion busy on a 15-file reading task; mail id 5 enqueued 00:49:27.08; `notify-agent!` returned false after 10387 ms (`wake consumption was not acknowledged; mail remains queued`); at the failure instant the queue file still held id 5 (`QUEUE-AT-RETURN` in-process capture); companion popped id 5 at 00:49:48.37 and confirmed consumption in-pane — retained through failure, consumed later, nothing lost | `36/wake-phase3-output.txt`, `36/to-agent-failedwake2-samples.log` |

| 2026-10-15 00:40–00:45 +0800 | #35 executed in fresh temp projects under `uml-acc-32/` (`uml acc test/proj arg (35)/` for argv/cwd; `proj-arg/proj-dep/` for dependency refusal), tool checkout cloned at 5e4407e in each; project A's running viewer/companion never touched. Complex argv (8 args with spaces/quotes/`;`/`&`/CJK/backslash/`$`/glob chars) invoked from foreign cwd `/tmp` through sh → pwsh → uml.ps1 → uml-command.ps1: byte-identical at the launch manifest AND at the launcher→JVM argv endpoint (recording shim in place of `clojure`; no real viewer launched). All three refusal scenarios rejected with exit 1 and actionable guidance: no pwsh (sh wrapper), Herdr 0.9.2 (preflight `select-herdr!`), protocol 21 schema (same) — zero launch manifests / JVM / Herdr side effects in the refusal runs | #35 section below; `35-argv-dep/` (s1c-shim-decoded.txt, launch-*.json, s1-byte-compare.txt, s2a/s2b/s2c-*.txt, shim sources) |
| 2026-10-15 01:01:28–01:01:52 +0800 | #38 phase A: SIGTERM×4 on companion children 83410→3958→4213→4583 (per-kill ps ownership verified, parent chain java runner → pane shell 83331); journal backoffs 1.095/2.05/4.11 s; attempts 1→2→3; `:failed` "Recovery exhausted after three retries" exit 143; no 4th relaunch in 12 s; controller 83418 exited | `38/phase-a-timeline.log`, `38/recovery-transitions.log`, `38/recovery.failed.edn`, `38/runner0.edn`–`38/runner-3.edn` |
| 2026-10-15 01:23–01:24 +0800 | #38 phase B attempt 1: re-arm via production `recovery/start!` hit the stale-generation-file race (harness artifact: same-session re-arm reuses `-runner-1.edn`, token mismatch → `"Runner generation changed"`); companion still exited 0 normally later; race snapshot kept | `38/phase-b-timeline.log`, `38/recovery.rearm-stale-race.edn` |
| 2026-10-15 01:29:02–01:29:36 +0800 | #38 phase B rerun (stale gen≥1 files cleaned): re-arm → controller 14684 → relaunch gen1 child 14717 → normal exit (ctrl+c, `:exit 0`) → journal `:normal` "Companion exited normally" in ~0.5 s; controller exited; 12 s watch: no runner-2, no new claude — zero recovery. Resumed companion drained retained mail id 5 on startup | `38/phase-b2-timeline.log`, `38/recovery.normal.edn` |
| 2026-10-15 01:31:40–01:32:03 +0800 | #38 phase C: screen is locked (`screenIsLocked=1`) so the red button cannot be synthesized; production close chain (`shutdown-children!`→`cleanup!`→`recovery/cancel!`) driven from a pre-warmed JVM inside the 4 s backoff after the 2nd SIGTERM; trigger→cleanup 705 ms; journal `:cancelled` "Viewer closed; recovery revoked" 3.9 s before deadline; controller 15663 exited; no zombie relaunch in 12 s; companion.edn `{}`; session deleted (only `default` remains); server 83305/shell 83331 gone | `38/phase-c-timeline.log`, `38/phase-c-canceller.log`, `38/recovery.cancelled.edn` |
| 2026-10-15 01:32–01:33:20 +0800 | #38 restoration: `:quit-for-restart` mail id 6 → viewer 83289 exited 0 (session.edn saved); fresh `./uml` healthy in 18 s — new session `uml-viewer-5f26e88b-06582e8d`, viewer 16347, companion claude 16494, controller 16502 armed, `:display` consumed (`next-id 8`, queue empty). Ready for #37/#40 | `38/phase-d-uml.out`, `38/phase-d-restored-health.txt` |
| 2026-10-15 01:42–01:43 +0800 | #37 setup: project-b worktree detached at 5e4407e; runtime clone installed via `get-uml-viewer --install-only` with `UML_VIEWER_REPO_URL`/`UML_VIEWER_REF` pointing at the local repo's spec branch (branch not pushed to qinyuanj); clone verified at 5e4407e |
| 2026-10-15 01:43:08–01:43:12 +0800 | #37 B cold start while A live: session `uml-viewer-7db6871f-aa6c2aa8`, viewer 19487 / controller 19644 / server 19507 / shell 19533 / runner 19630 / claude 19634, watching `examples/library.edn`; A's companion.edn md5 and process table untouched | `37/b-launch.out`, `37/b-uml-viewer-log.txt`, `37/b-pre-operation.txt` |
| 2026-10-15 01:48 +0800 | #37 isolation op on A: production `write-command!` + `notify-agent!` wake (mail id 6 `:context :real`) → `WAKE-RESULT true`, A queue drained, A companion confirmed in-pane; B's companion.edn/recovery.edn md5, mailboxes, six process lstarts and pane content all identical before/after; `herdr session list` shows A+B running, `default` untouched | `37/a-wake-operation.txt`, `37/a-pane-after-wake.txt`, `37/b-pre-operation.txt`, `37/b-post-operation.txt` |
| 2026-10-15 01:54:31 +0800 | #40 pre-state: `.metrics/` + diagram backed up; quality-result.json absent; mailboxes idle; screen still locked (screenshot substitution = CGWindowList) | `40/pre-state.txt`, `40/pre-metrics-backup/` |
| 2026-10-15 01:56:13–01:58:07 +0800 | #40 FIFO batch `[7 refresh-mutate, 8 refresh-crap, 9 refresh-mutate-all, 10 regen]` on A companion (target `:domain.mailbox`): mutate exit 0 validResult true → IR → display (id 8, deferred until after CRAP — companion-disclosed 顺序偏差, content gated); crap exit 1 validResult false (coverage exit 9 inside, #47) → rejected, no IR/display; mutate-all exit 2 (baseline FAIL, #47) → rejected, no IR/display; regen IR exit 0 → display id 9. Both displays consumed (`next-id 10`); op windows disjoint in 2 s process sampling; `.metrics/` and diagram content byte-identical afterwards | `40/serial-samples.log`, `40/mut7.log`, `40/crap8.log`, `40/mut9.log`, `40/quality-result-final.json`, `40/companion-pane-final.txt`, `40/post-health.txt` |
| 2026-10-15 02:10:20–02:11:00 +0800 | #41 phase A: production `request-agent!` injected mail 11 `:omit {:id :domain.mailbox}` (no `:proposal-id`); companion added `:omit [:domain.mailbox]` to policy (02:10:43) → `./uml ir` regenerated EDN (02:10:45, single-line `:omit` diff, md5 a9ab83d6→9d2c77d4) → `:display` (to-viewer next-id 10→11, consumed); proposals preserved | `41/pre-state.txt`, `41/policy-pre.edn`, `41/diagram-pre.edn`, `41/phase-a-omit-real.txt`, `41/phase-a-pane.txt` |
| 2026-10-15 02:16:05–02:17:00 +0800 | #41 phase B: mail 12 `:omit {:id :domain.policy, :proposal-id :p-1791033119418}`; companion added `:omit [:domain.policy]` inside proposal `test` (top-level omit untouched, proposal id/name/layers preserved) → IR exit 0 → display id 11 consumed | `41/phase-b-omit-proposal.txt`, `41/phase-b-pane.txt` |
| 2026-10-15 02:18–02:22 +0800 | #41 phase C: mail 13 `:context :proposal :p-1791033119418 "test"` acknowledged ("直到下一条 :context 到来", no IR/display); mail 14 `:context :real` switched back; queues drained (TA next-id 15); A/B healthy, B untouched (policy md5 = A pre-run) | `41/phase-c1-pane-context-proposal.txt`, `41/phase-c2-pane-context-real.txt`, `41/post-state.txt`, `41/screen-lock.txt` |
| 2026-10-15 02:26:50–02:26:51 +0800 | #43 A close via production `shutdown-children!` (red-button substitute under locked screen, windowClosing not triggered): 1.27 s, recovery `:cancelled` ("Viewer closed; recovery revoked"), companion processes 16364/16394/16481/16494/16502 gone, herdr session + socket dir deleted, companion.edn → `{}`; 16 s +7 min no-revival watch clean; viewer JVM 16347 SIGTERM (halt-vm! substitute, no shutdown hooks) gone in 1 s | `43/pre-state.txt`, `43/a-post-close.txt`, `43/no-revival-watch.txt`, `43/final-state.txt` |
| 2026-10-15 02:28–02:34 +0800 | #43 B survival: six process lstarts + companion/recovery/to-viewer md5 + pane bytes all identical across A's close; harmless `:context :real` wake on B consumed (TA next-id 1→2, pane ack) — B functional; ambiguous-identity probe: re-run `shutdown-children!` on A's `{}` record → explicit "Companion ownership is missing, ambiguous or belongs to another project", zero side effects. A left closed (saved session.edn intact) for #39 | `43/b-survival-static.txt`, `43/b-pane-pre.txt`, `43/b-pane-post-close.txt`, `43/b-pane-after-wake.txt`, `43/ambiguous-identity-probe.txt` |
| 2026-10-15 02:40–02:47 +0800 | #39 refusal battery: temp roots (`/tmp/uml-acc-39/`, symlinked tool) — missing companion → preflight refusal; cross-project record → "belongs to another project; file retained"; launcher identity changed (r4) / no JVM record (r5) / JVM identity changed (r6) → exact wrapper refusals, exit 1, zero launch artifacts. Real A `--restart` with `companion.edn {}` → "Companion ownership is missing; cannot restart", no side effects | `39/r1.out`, `39/r4.out`, `39/r5.out`, `39/r6.out`, `39/a-restart-refusal.out`, `39/pre-state.txt` |
| 2026-10-15 02:47→08:37 +0800 | #39 fresh `./uml` revives A (session uml-viewer-5f26e88b-**bcc99ab2**, claude 14640); host slept ~6 h mid-launch (no effect); display id 12 consumed, recovery `:running` | `39/viewer-log.txt`, `39/final-state.txt` |
| 2026-10-15 08:39–08:43 +0800 | #39 restart cycles ×3 (JVM 14466→15691→16516→17316), each binding the SAME companion (session bcc99ab2, claude 14640, no new session). View restore round-trips: namespace tree + `:discussion-context {:context :real}` retained, mail-seen 14→15→16 cursor preserved; named-proposal view (`:p-1791033119418`, augmented via production `write-session!` — GUI click locked out) validated + retained, `:proposal` false→true. Five functions post-combo: `:context` mail 15 consumed; FIFO displays 17/18 drained in order; `:regen`→IR exit 0→display 19 consumed; `./uml ir` exit 0 mtime bump with viewer healthy; metrics overlay rejoin error-free (visual check locked out, noted) | `39/session-roundtrips.txt`, `39/session-final.edn`, `39/companion-pane-final.txt`, `39/a-restart1.out`–`a-restart3.out`, `39/a-ir-mtime2.out` |
| 2026-10-15 09:00 +0800 | #44 consolidation (PASS path, this file finalized): header/environment updated to 5e4407e/c2b3c5c, one-line verdict table added (#33–#43 PASS, attach NOT VERIFIED → #49), #34 header updated to final PASS, terminal-attach handoff moved #46→#49, 问题清单 rewritten (#47, #48, #40 timing, probe-herdr "unknown", wrapper exit 0, Terminal -10006 → #49, host-sleep note; #45 resolved) | this file |

## Per-criterion results

Legend: PASS / FAIL / NOT VERIFIED, evidence pointer (relative to
`acceptance-evidence/`), and the key command with its exit code. The
one-line summary below is the verdict of record; the per-ticket sections
that follow carry the full detail. #34's section documents the full
FAIL→fix→PASS history (three re-runs, defect #45); its final verdict is
PASS. Locked-screen substitutions are noted per criterion: no screenshots
(CGWindowList instead), no window-chrome events (`shutdown-children!` for
the red button, `write-session!` for the inspector proposal click).

| Ticket | Criterion (short) | Verdict | Key command → exit | Evidence |
|---|---|---|---|---|
| #33 | Skeleton + environment record | PASS | worktree/env capture → 0 | env table above |
| #34 | Cold start, companion created exactly once | PASS (initial runs FAIL → #45; PASS at 5e4407e) | `./uml` → 0; `:display` consumed; herdr inventory = exactly one new session | `34-rerun3/` (see #34 section) |
| #35 | argv/cwd passthrough + dependency refusal | PASS | `./uml` scenarios a–c → 1 each with exact guidance; argv byte-compare | `35-argv-dep/` (see #35 section) |
| #36 | FIFO mail semantics | PASS | `write-command!`×3 → drains oldest-first; failed wake retains mail | `36/` (see #36 section) |
| #37 | A/B true parallelism + isolation | PASS | B cold `./uml` while A live → 0; op on A leaves B byte-identical | `37/` |
| #38 | Abnormal-exit recovery 1/2/4 s ≤3; normal exit none; close cancels | PASS | SIGTERM×4 → backoffs 1.095/2.05/4.11 s; `:normal`/`:cancelled` journals | `38/` |
| #39 | wrapper `--restart` + view restore | PASS | `./uml --restart` → 0 ×3, same companion; refusal battery → 1 ×6 | `39/` |
| #40 | Quality dispatch serial discipline | PASS (display-timing observation → 问题清单) | `./uml crap` → 1 rejected; `./uml mutate` → 0 pass / → 2 rejected; IR → 0 gates display | `40/` |
| #41 | `:omit` / `:context` flows | PASS | `request-agent!` mails → policy `:omit` → `./uml ir` → 0 → display; context pinned | `41/` |
| #42 | Legacy tmux record retained + refused | PASS | `./uml` (main repo) → refusal, zero side effects | main-repo snapshot table |
| #43 | GUI close cleans own resources; B survives; ambiguous identity refuses | PASS | `shutdown-children!` → 1.27 s; 16 s+7 min no-revival; probe → explicit refusal | `43/` |
| attach | Terminal attach (manual) | NOT VERIFIED → #49 | not executed (AFK, locked screen) | this section |


### #34 Cold start — fresh companion created exactly once — PASS (initial runs FAIL → defect #45; passed at 5e4407e after three fixes)

History: the first three runs (2026-10-14) failed at platform level —
defect filed as **issue #45**. Control experiment per spec #32 (0.9.3
fallback): **0.9.3 failed identically** (3/3 runs across 0.9.4 ×2 and
0.9.3 ×1, same server-log signature). The failure was macOS-platform
level, not a 0.9.4 regression. Three companion fixes landed during
acceptance (0b6f1ea session-name sun_path, 6355aa4 login-shell detection,
5e4407e canonical-mode retry); re-run 3 at 5e4407e **passed** (see the
re-run subsections below) and #45 is closed.

Evidence: `34-uml-invocation.txt`, `34-coldstart-watch.log`,
`34-failure/` (first 0.9.4 run), `34-failure/repro-2/` (accidental
second 0.9.4 run — also proves ADR 0004 highest-compatible selection),
`34-uml-invocation-0.9.3.txt`, `34-coldstart-watch-0.9.3.log`,
`34-failure/control-0.9.3/` (0.9.3 isolated-binary control).

Criterion 1 (`./uml` cold start reaches `:display`, command + exit code
logged): **FAIL**. `./uml` itself exited 0 (2 s, launcher pid 28673), but
the viewer JVM died 2 s later: Herdr 0.9.4 server (pid 28696, session
`uml-viewer-5f26e88b-7cabe77b-4ddb-4d0b-8194-481212cfec2f`) exited during
the first `workspace create` API call with `server_unavailable: receiving
on a closed channel`; its own log records `api.request.complete
outcome=error` for `workspace.create` immediately followed by
`app.shutdown outcome=completed` — the server, not the client, terminated
the exchange. The JVM exited 1 (`uml-viewer-log.txt` stack trace);
`:display` never arrived within the 195 s watch; no viewer window, no
surviving java/herdr processes.

Criterion 2 (companion.edn vs Herdr session inventory proves single fresh
creation): **NOT VERIFIED** (blocked by criterion 1). Partial observation
from all three runs: each launch created exactly one new Herdr session
(none existed at baseline; names carry fresh UUIDs) and wrote
companion.edn exactly once per launch, with the provisional record
retained on abort exactly as the contract prescribes — but every run
stopped at `:provisional true`, so uniqueness of a *completed* fresh
creation is unproven.

Failure-site handling (team-lead authorized): after copying evidence to
`34-failure*/`, each run's residue (provisional companion.edn,
viewer-process.json, uml-viewer-log.txt) was removed and its stopped
session deleted via `herdr session delete` (exit 0); inventory returned
to baseline (`default` only) before each subsequent run. The 0.9.3
control run's residue was cleaned the same way afterwards; project A is
back to a clean baseline (only the installed runtime under
`.uml-viewer/`).
Noteworthy correlation recorded in #45: in all three server logs
`update.check.start` fired between `api.request.start` and the erroring
`api.request.complete` for `workspace.create`.

#### #34 re-run after the #45 fix (0b6f1ea, 2026-10-15) — FAIL at a new, later site

#34 was reopened after `0b6f1ea fix(companion): shorten Herdr session
names to fit macOS sun_path` landed; the re-run executes the identical
cold-start procedure on the fixed code (worktree and runtime clone both
at 0b6f1ea).

**The #45 fix itself is validated.** The fresh session is
`uml-viewer-5f26e88b-8c637e27` — 28 chars, the new shortened format;
its socket paths measure 76 B (`herdr.sock`) and 83 B
(`herdr-client.sock`), both comfortably under the 104-byte macOS
`sun_path`. The Herdr 0.9.4 server (pid 78964) survived the full
provisioning sequence: `workspace create` — the call whose error
signature killed the server in all three 2026-10-14 runs — completed
with `outcome=ok` in the server log, and the server persisted the
session afterwards (`persist.save outcome=ok`). The server and pane
shell were still alive minutes later.

**But criterion 1 still FAILs at a new, later failure site.** The
viewer JVM died (exit 1) at `create!` step "inject runner command into
pane" (`herdr.clj:272`):

```
clojure.lang.ExceptionInfo: Unsupported Herdr pane shell; command was
not sent; ownership retained {:status :failure, :shell "-zsh"}
```

Root-cause chain (all evidence in `34-rerun/`): `process-owner!`
(`herdr.clj:58-66`) identifies the pane shell via `ps -p <pid> -o
lstart= -o comm=`; on macOS a login shell's `comm` keeps the leading
dash (`ps -p 78990 -o comm=` → `-zsh`, verified live and in
`ps-shell-comm.txt`; herdr's own `pane process-info` likewise reports
`argv ["-zsh"]` / `cmdline "-zsh"`, while its parsed `name` field is a
clean `zsh`). `pane-command!` (`herdr.clj:161-168`) then lowercases the
basename and matches against `#{"sh" "bash" "zsh" "dash"}` — `-zsh`
misses, so it `fail!`s before any command is sent. This defect was
latent, not introduced by 0b6f1ea: every earlier macOS run died at
`workspace create` and never reached the pane-command step. zsh login
shells are the macOS default, so the site is expected to reproduce on
any standard macOS setup.

Criterion 2 (single fresh creation): the partial-observation pattern
from 2026-10-14 repeated — exactly one new Herdr session with a fresh
name, companion.edn written once and retained at `:provisional true`
after the abort exactly as the contract prescribes — but a *completed*
fresh creation remains unproven: **NOT VERIFIED**.

Failure-site handling: per the stop rule, **no cleanup was performed**
— the running session `uml-viewer-5f26e88b-8c637e27` (server pid
78964, pane shell pid 78990), provisional companion.edn,
viewer-process.json and uml-viewer-log.txt are all left in place for
debugging; evidence was copied to `34-rerun/` first. Issue #34 stays
open; the new defect and the go/no-go decision are reported to the
team lead.

#### #34 re-run 2 after the login-shell fix (6355aa4, 2026-10-15) — FAIL at a third distinct site

The team lead fixed the `-zsh` rejection as
`6355aa4 fix(companion): accept macOS login shells in Herdr pane command
detection` (login-shell argv0 dash stripped at both call sites, 2 new
spec tests green) and cleaned the re-run 1 scene; the identical
cold-start procedure was executed again with worktree and runtime clone
both at 6355aa4.

**The login-shell fix is validated** — `pane-command!` accepted the
`-zsh` pane shell and the flow advanced past herdr.clj:272 into the
actual `pane run` delivery. Session `uml-viewer-5f26e88b-999df526`
(28 chars), server pid 81480, `workspace create` ok again.

**Criterion 1 still FAILs at a third, again later, failure site** —
`pane run` itself was rejected by the Herdr server:

```
pane_send_failed: input contains a 3130-byte line but the terminal is
in canonical mode with a 1024-byte limit; wait for the shell prompt and
retry
```

The runner command is a single-line `java -cp <30+ jars> clojure.main
-m uml-viewer.adapters.recovery runner <manifest>` invocation measuring
3130 bytes. A freshly spawned macOS PTY is in canonical mode
(MAX_CANON = 1024) until the shell brings up its line editor; the
adapter delivers the command immediately after pane spawn with no
shell-readiness wait, so the line exceeds the limit and
`pane.send_input` errors (server log: `api.request.complete
outcome=error method=pane.send_input`; the server itself stayed alive
and persisted the session). The herdr error text explicitly prescribes
"wait for the shell prompt and retry" — the adapter has no such wait
on this path. JVM exit 1, `:display` never arrived, companion.edn
retained at `:provisional true` per contract.

Criterion 2: same partial pattern — exactly one new session, one
companion.edn write, provisional record retained on abort — completed
fresh creation still unproven: **NOT VERIFIED**.

Failure-site handling: per the stop rule, **no cleanup was performed**
— session `uml-viewer-5f26e88b-999df526` (server pid 81480 and its
pane shell), provisional companion.edn, viewer-process.json,
uml-viewer-log.txt and both runner launch manifests
(`...-8c637e27-...` leftover plus `...-999df526-...`) are left in
place; evidence copied to `34-rerun2/` first. Issue #34 stays open;
the third defect is reported to the team lead.

#### #34 re-run 3 after the canonical-mode retry fix (5e4407e, 2026-10-15) — **PASS**

The team lead fixed the canonical-mode rejection as
`5e4407e fix(companion): retry Herdr pane run while a fresh pty is in
canonical mode` (new `pane-run!` retrying only the canonical-mode
refusal, 10 s budget at 100 ms, both delivery sites unified; two new
spec tests green) and cleaned the re-run 2 scene; the identical
cold-start procedure was executed a third time with worktree and
runtime clone both at 5e4407e.

Criterion 1 (`./uml` cold start reaches `:display`, command + exit code
logged): **PASS**. `./uml` exit 0 (launcher pid 83284). The viewer JVM
(pid 83289, started 00:25:38) completed companion creation end-to-end
at 00:25:43 — the first completed Herdr fresh creation on macOS, after
the three-defect chain (#45 sun_path → login-shell name → canonical
line limit) was fixed one site at a time. The companion (claude
session `b7e464ee-…`, pid 83410 in pane w1:p1) worked 59 s, ran
`./uml ir` (exit 0), regenerated `examples/uml-viewer.edn`
(00:26:29) and sent `:display` (mail id 1); the viewer consumed it —
`to-viewer.edn` = `{:next-id 2, :queue []}` at 00:26:36, and the
on-screen window `UML viewer` (CGWindowID 18484, 1470×930 @(129,107))
holds the diagram. Launch → `:display` consumed: ≈72 s. One non-fatal
deviation recorded: the Terminal tab-title osascript failed `-10006`,
so `:window-id` is nil and the log prints the manual-attach guidance;
the companion itself was retained and is fully functional.

Criterion 2 (companion.edn vs Herdr session inventory proves single
fresh creation): **PASS**. Baseline inventory was `default` (stopped)
only; after the run exactly one `uml-viewer-*` session exists
(`uml-viewer-5f26e88b-6fc7dc58`, 28-char post-fix name, running).
companion.edn was produced by one `create!` flow (provisional writes
then one final write, birth=mtime 00:25:43, `:provisional false`,
`:backend :herdr`, all contract keys present); one wrapper launch; no
`launch-*.json` files; recovery.edn generation 0 / attempts 0. The
pane holds exactly the one companion (claude pid 83410) and its runner
(java pid 83395) — no duplicates.

GUI screenshot caveat: per-window `screencapture` evidence could not be
produced — the agent host lacks the Screen Recording TCC grant
(`could not create image from window`; a full-screen probe is fully
black). Window presence is instead evidenced by CGWindowList
enumeration (`34-rerun3/window-enumeration.txt`). This contradicts the
2026-10-14 precheck row for screen recording; that row's own caveat
covered this case.

Viewer and companion are **left running** for the follow-up tickets
(#35–#41/#43 re-execution), as instructed. Observation for the team:
the companion terminal's input box holds an unsubmitted draft
(「重启 viewer 看看新图」) not typed by the acceptance agent — likely
typed directly in the Terminal window; left untouched.

Evidence: `34-rerun3-uml-invocation.txt`, `34-rerun3-coldstart-watch.log`,
`34-rerun3/` (companion.edn.final, companion-stat.txt, recovery.edn,
status-server.json, session-list.json, api-snapshot.json,
to-viewer.edn.consumed, uml-viewer-log.txt, companion-pane-read.txt,
window-enumeration.txt).

### #35 Complex argv/cwd passthrough + missing/incompatible dependency refusal — PASS

Executed 2026-10-15 00:40–00:45 +0800 on macOS 27.0.1 (pwsh 7.6.7,
Clojure CLI 1.12.6.1673). Two fresh temp projects were created under
`/Users/billyq/Dev/unclebob/uml-acc-32/`, each with `uml` /
`uml.ps1` copied from the main-repo working tree and a tool checkout
cloned into `.uml-viewer/uml-viewer/` at 5e4407e (same commit as project
A). Project A's running viewer/companion (session
`uml-viewer-5f26e88b-6fc7dc58`) was never touched; all evidence lives in
`acceptance-evidence/35-argv-dep/` (copies) and the retained temp dirs
`uml-acc-32/proj-arg/` + `uml-acc-32/uml acc test/`.

**Criterion 1 — complex argv/cwd passthrough: PASS.**

Project path itself contains spaces and parens:
`/Users/billyq/Dev/unclebob/uml-acc-32/uml acc test/proj arg (35)`.
Invocation was made from a foreign cwd (`/tmp`), by absolute path, through
the full chain sh `./uml` → `pwsh -File uml.ps1` → `uml-command.ps1` →
launch manifest → detached `uml-launch.ps1` → JVM argv. To avoid launching
a real viewer, a recording shim replaced `clojure` on PATH
(`proj-arg/shims/clojure-fake/clojure`): it logs cwd + base64 argv of
every invocation, answers the preflight with the required single line
`herdr`, and exits 42 for the viewer launch. Input argv after
`--backend herdr` (8 args, chosen to break naive re-splitting):
`arg with spaces`, `quote'inside`, `double"quote`, `semi;colon&amp`,
`中文参数`, `back\slash`, `dollar$HOME`, `star*glob?`.

- Wrapper exit 0; stdout:
  `UML viewer starting (launcher pid 92613). Log: .../uml acc test/proj arg (35)/uml-viewer-log.txt`
  (`s1c-stdout.txt`; two earlier runs pids 88083/92336 same shape).
- Launch manifest `launch-21797ee2866546e8b3be560c5b816b16.json`
  (snapshotted mid-flight by a `find`-watcher before `uml-launch.ps1`
  deleted it; `manifest-snap/`): `cwd` = the space/paren project path,
  `argv` tail byte-identical to the 8 input args (UTF-8 byte compare
  8/8 IDENTICAL, `s1-byte-compare.txt`); the `-Sdeps` JSON embeds the
  space/paren path intact.
- Shim log, decoded (`s1c-shim-decoded.txt`): BOTH the preflight
  invocation (argc 17, includes root and `Mac OS X`) and the launcher →
  JVM invocation (argc 15) ran with cwd = the space/paren project path
  (invocation cwd `/tmp` never leaked in), and carried all 8 special args
  char-for-char (scripted compare: PASS 8/8 at the JVM endpoint).
- Chain hygiene: the launcher consumed and deleted the manifest (0
  residual `launch-*.json`); `viewer-process.json` records the detached
  launcher (pid 92613); no JVM was ever started (shim exit 42 went to
  `uml-viewer-log.txt` only). Evidence: `s1-environment-and-final-state.txt`.

**Criterion 2 — dependency refusal, three scenarios: PASS ×3.**

All in `proj-arg/proj-dep/` (no companion.edn; fresh-start path).
Verbatim stderr in `s2a/s2b/s2c-stderr.txt`; all stdout empty.

| Scenario | Command | Exit | stderr (verbatim, abridged where noted) | Guidance? |
|---|---|---|---|---|
| a. no pwsh | `env PATH=/usr/bin:/bin:/usr/sbin:/sbin ./uml` | **1** | `uml: PowerShell 7 (pwsh) is required. Install: https://aka.ms/powershell-install` | yes — install URL |
| b. Herdr 0.9.2 | `./uml` with exclusive-PATH shim dir (`shims/herdr-0.9.2/`: fake `herdr` reporting `herdr 0.9.2` + pwsh/clojure/clj/git symlinks; real 0.9.4 unreachable) | **1** | `uml: Herdr 0.9.3 or newer (protocol 22) is required. Install a release from https://github.com/herdrdev/herdr/releases (https://herdr.dev). No automatic download, upgrade or fallback. Windows native acceptance is partial; macOS has not passed native acceptance. Candidates on PATH: .../shims/herdr-0.9.2/herdr (unknown: not a herdr X.Y.Z >= 0.9.3)` | yes — minimum version, releases URL, per-candidate reason |
| c. protocol ≠ 22 | same but `shims/herdr-proto21/herdr` reports `herdr 0.9.3` and `api schema --json` answers `"protocol":21` | **1** | same guidance prefix; candidate listed as `(0.9.3: bundled schema is not protocol 22)` | yes — protocol named in guidance + reason |

Refusal points: (a) the sh wrapper `uml:3-6`, before pwsh exists; (b)/(c)
the read-only preflight JVM `uml-viewer.adapters.companion` →
`select-herdr!`/`probe-herdr` (companion.clj:110-132), before any launch
manifest, viewer JVM or Herdr call. Zero-side-effect evidence: after
(b) and (c), `proj-dep/.uml-viewer/` contains only the tool checkout — no
`launch-*.json`, no `viewer-process.json`, no `uml-viewer-log.txt`;
`pgrep` shows no stray processes. Minor cosmetic observation (not a
failure): in (b) the rejected candidate is listed as `unknown` because
`probe-herdr` drops the parsed `:release` on the version-rejected branch
(companion.clj:113); scenario (c) shows the version correctly.

The known deviation first recorded under #42 stands and was **not**
re-tested here: with the tool checkout missing entirely, the wrapper
prints a pwsh error yet exits 0 (`uml.ps1` `exit $LASTEXITCODE` with null
`$LASTEXITCODE`). All #35 runs installed the checkout first, per the
documented setup.

Both acceptance criteria PASS: argv/cwd evidence recorded byte-exact; all
three dependency problems refused with exit 1, guidance-bearing messages,
and zero side effects.

### #36 On-diagram FIFO mail semantics — PASS (executed 2026-10-15 on the running 5e4407e instance)

Executed against the live project-A viewer (JVM 83289) and companion
(session `uml-viewer-5f26e88b-6fc7dc58`) left running by #34 re-run 3.
All queue writes used the documented mailbox format (atomic tmp+rename);
companion wakes used the production `notify-agent!` code path (loaded
from source in a throwaway Clojure JVM, `clojure -Sdeps quil -M -e`).
No viewer restart/close operations were performed. Evidence directory:
`36/`.

Criterion 1 (multiple mails consumed oldest-first, mail-file diff
sequence): **PASS**, both directions.

- **to-viewer** (`36/to-viewer-fifo-samples.log`): three `:display`
  mails enqueued atomically at 00:41:51.562 (ids 2,3,4 = library.edn,
  uml-viewer.edn, library.edn). The viewer popped strictly head-first,
  rewriting the file per pop, applying each display between takes:
  `[2,3,4]` →(+8 ms)→ `[3,4]` →(+92 ms)→ `[4]` →(+110 ms)→ `[]`.
  A fourth restore mail (id 5, uml-viewer.edn) was consumed in 220 ms
  at 00:42:18, leaving the acceptance diagram on screen; viewer JVM
  healthy throughout.
- **to-agent** (`36/to-agent-fifo-samples.log`,
  `36/companion-pane-final.txt`): three distinct `:context` mails
  enqueued via production `write-command!` (ids 1,2,3 = `:real`,
  `:proposal p-1791033119418 "test"`, `:real`); woken via production
  `notify-agent!` (herdr `agent prompt`, wake token). The companion
  popped the queue and reported in-pane: 「已按序处理 3 条 :context
  邮件：real → proposal "test" → real」 — explicit oldest-first
  order; queue empty afterwards (`{:next-id 4, :queue []}`).
  The wake's 10 s acknowledgement window expired before the companion
  finished (~21 s of work), so `notify-agent!` returned false with
  "mail remains queued" while the mail was in fact retained and then
  consumed — an incidental first retention observation.

Criterion 2 (queue retained on failed wake): **PASS**
(`36/wake-phase3-output.txt`, `36/to-agent-failedwake2-samples.log`).
With the companion busy on a code-reading task, mail id 5
(`:context :real`) was enqueued and `notify-agent!` called with
in-process timestamps: `WAKE-RESULT false elapsed-ms 10387` —
stderr `companion wake failed: Herdr wake consumption was not
acknowledged; mail remains queued; ownership retained` — and at the
moment of return the queue file still held the mail:
`QUEUE-AT-RETURN {:next-id 6, :queue [{:id 5, ...}]}`.
Retention is therefore proven at the failure instant, not inferred
afterwards. The companion later popped id 5 at 00:49:48 (21 s after
enqueue) and confirmed consumption in-pane; no mail was lost and the
queue drained normally once the companion was reachable.

Note for the record: the companion terminal's input box held an
unsubmitted human-typed draft throughout (its text changed between
phases; never submitted); herdr `agent prompt` submits independently of
the draft, so the mail flow was unaffected.

### #37 A/B true parallelism and isolation — PASS (executed 2026-10-15, both projects at 5e4407e)

Setup: project-b worktree was at c714f83 with no runtime clone; detached at
5e4407e (commit already in the shared local object store — the branch exists
only locally, qinyuanj's `lua` is c714f83), then installed the runtime clone
with `get-uml-viewer --install-only` overriding
`UML_VIEWER_REPO_URL=/Users/billyq/Dev/unclebob/uml-viewer` and
`UML_VIEWER_REF=spec-32/macos-herdr-acceptance`; clone verified at 5e4407e
(the viewer JVM runs from the runtime clone, so this matters).

**Criterion 1 — A/B simultaneously running, clear ownership — PASS.** B
cold-started (`./uml`) while A was fully live, healthy in ~4 s. Identity
table (evidence `37/a-pre-state.txt`, `37/b-pre-operation.txt`):

| | A | B |
|---|---|---|
| session | uml-viewer-5f26e88b-**06582e8d** | uml-viewer-7db6871f-**aa6c2aa8** |
| viewer JVM | 16347 | 19487 |
| controller | 16502 (child of 16347) | 19644 (child of 19487) |
| herdr server | 16364 | 19507 |
| pane shell | 16394 | 19533 |
| runner → claude | 16481 → 16494 | 19630 → 19634 |
| cwd / mailboxes | project-a/.uml-viewer | project-b/.uml-viewer |
| socket | ~/.config/herdr/sessions/…-06582e8d/ | ~/.config/herdr/sessions/…-aa6c2aa8/ |

Session-name first segments differ per project (5f26e88b vs 7db6871f);
`herdr session list` shows both running with the pre-existing stopped
`default` session untouched. Mailboxes are per-root files with independent
cursors (A to-agent next-id 7, B has no to-agent yet; A to-viewer next-id 8,
B next-id 2). B's launcher printed the same harmless "terminal open failed;
companion retained" Terminal-AppleScript note (locked console) — companion
fully controllable via herdr CLI, as with A.

**Criterion 2 — operating on A leaves B's processes and state files
unchanged — PASS.** Operation on A via the production path (throwaway JVM):
`write-command!` appended `{:id 6, :op :context, :context :real}` to A's
to-agent.edn and `notify-agent!` woke A's companion — `WAKE-RESULT true`
(consumption acked inside the 10 s window), A's queue drained, and A's
companion confirmed in-pane 「我处理了 1 条邮件：{:id 6 :op :context …}」
(`37/a-wake-operation.txt`, `37/a-pane-after-wake.txt`). B before/after
(`37/b-pre-operation.txt` vs `37/b-post-operation.txt`): companion.edn md5
`9bc9d904…` unchanged, recovery.edn md5 `70adaa3c…` unchanged, mailboxes
unchanged (to-viewer `{:next-id 2, :queue []}`, to-agent still absent), all
six B processes alive with original lstart, pane content unchanged. Reverse
direction also held: B's entire cold start left A's companion.edn md5
`309ed71f…` and A's process table untouched.

Final state: A and B both healthy (A for #40, B kept running for #43).

### #38 Abnormal-exit recovery (SIGTERM, 1/2/4 s budget, max 3) and normal-exit boundary — PASS (executed 2026-10-15 on the 5e4407e instance, session uml-viewer-5f26e88b-6fc7dc58)

All signals were SIGTERM only, each preceded by `ps` ownership verification
(child pid read from that generation's runner state file; `comm` must be
`claude`; parent chain java runner → pane shell 83331). No SIGKILL, no
other session touched; the acceptance session and the stopped `default`
Herdr session were never signalled.

**Criterion 1 — abnormal exit, 1/2/4 s backoff, max 3 retries — PASS.**
Four SIGTERMs on the companion claude children (gen0 83410 → gen1 3958 →
gen2 4213 → gen3 4583). Controller-observed backoffs from the journal:
1.095 s / 2.05 s / 4.11 s (matches `backoff-ms [1000 2000 4000]`;
wall-clock deltas include ~1–2 s runner JVM boot). `attempts` progressed
1→2→3; after the 3rd kill the journal went `:phase :failed` with
`"Recovery exhausted after three retries"` and `:exit 143`; no 4th
relaunch appeared within a 12 s watch and controller 83418 exited.
Evidence: `38/phase-a-timeline.log`, `38/recovery-transitions.log`,
`38/recovery.failed.edn`, `38/runner0.edn`–`38/runner-3.edn`.

**Criterion 2 — normal exit triggers zero recovery — PASS.**
Re-armed via production `recovery/start!` (the `:failed` journal was
retired by `retire-completed!`); controller 14684 relaunched the companion
(gen1 child 14717, ownership verified) after the 1 s backoff. A normal
exit was then driven at the idle prompt (ctrl+c sequence; runner recorded
`:exit 0`). The controller verdicted `:phase :normal`
`"Companion exited normally"` within ~0.5 s and exited (14684 GONE); a
12 s watch saw no runner-2 file and no new claude process. The resumed
companion also drained the retained `{:id 5 :op :context :real}` mail on
startup, confirming "recovery resumes this Claude session and drains
queued mail". Evidence: `38/phase-b2-timeline.log`,
`38/recovery.normal.edn`, `38/phase-b2-pane-before-exit.txt` /
`-after-exit.txt`.

Harness note (not a product defect): re-arm attempt 1
(`38/phase-b-timeline.log`) hit a stale-file race — generation files are
`<sid>-runner-N.edn`, write-once per lifecycle by design; a same-session
re-arm reuses generation 1 and the controller read the previous
lifecycle's file (token mismatch → `"Runner generation changed; exit
cause unknown"`). Production never re-arms over the same session
(`start!` is only called by `create!` with a fresh session name;
`--restart` only verifies), so this is a harness artifact; cleaning stale
gen≥1 runner files before re-arm (phase-b2) avoids it. Race snapshot:
`38/recovery.rearm-stale-race.edn`, `38/runner-1-phaseb1-normal-exit0.edn`.

**Criterion 3 — viewer close during a pending retry cancels recovery, no
zombie relaunch — PASS via the production shutdown chain, with an
environmental caveat.** The host's console session is **screen-locked**
(`CGSessionCopyCurrentDictionary` → `screenIsLocked=1`, `loginwindow`
frontmost, Display Shield window present), which makes a literal
red-button GUI close impossible to synthesize: AX exposes no close
button/raise/main attributes for the java window, CGEventPostToPid cannot
deliver chrome clicks (WindowServer hit-testing), a global synthetic click
would hit the user's covering Ghostty window, and no keyboard quit path
exists (no `QuitHandler`/`addShutdownHook` — a ⌘Q default-quit would
bypass `cleanup!` entirely; the window red button is the only wired close
path). The locked screen also retroactively explains the black
screenshots in the #34 era. Instead, the exact production chain the red
button invokes (`on-main-close` → `exit-app!` → `shutdown-children!` →
`companion/cleanup!` → `recovery/cancel!`) was executed from a pre-warmed
throwaway JVM during the **4 s backoff window** after the 2nd SIGTERM
(attempts=2). Trigger→cleanup-done took 705 ms; the journal went
`:phase :cancelled` with `"Viewer closed; recovery revoked"` 3.9 s before
the relaunch deadline; controller 15663 exited; no zombie relaunch within
12 s; companion.edn was cleared to `{}`; herdr server 83305 and pane shell
83331 terminated; `herdr session list` shows only the untouched `default`
session. Evidence: `38/phase-c-timeline.log`, `38/phase-c-canceller.log`,
`38/recovery.cancelled.edn`. The AWT `windowClosing` dispatch itself is
the only unexercised link and can only be exercised by a human clicking
the red button on an unlocked console.

**Criterion 4 — healthy final state for #37/#40 — PASS.** The old viewer
consumed `{:id 6, :op :quit-for-restart}` and exited 0 (session.edn saved,
`mail-seen 6`). A fresh `./uml` came up healthy in 18 s: new session
`uml-viewer-5f26e88b-06582e8d` (28-char name), viewer JVM 16347, companion
claude 16494 (runner 16481 ← pane shell 16394), controller 16502 armed
(`:phase :running`), `:display` mail consumed (`{:next-id 8, :queue []}`).
Evidence: `38/phase-d-uml.out`, `38/phase-d-restored-health.txt`.

### #39 Public-wrapper `--restart` — old JVM exit / new JVM bind / companion continuity / view restore — PASS (executed 2026-10-15, project A)

Starting point: A fully closed by #43 (`companion.edn {}`, saved
`session.edn`, recovery `:cancelled`). Note: the host slept ~6 h
(02:47→08:37) mid-launch of the fresh-start JVM; all timestamps below are
post-wake and the pause had no effect on results. Screen remained locked
(`screenIsLocked=1`); GUI-only interactions (inspector proposal click) are
substituted with production state writers, noted per arm. Evidence: `39/`.

**Criterion 1 — old JVM exit, new JVM binds the same companion — PASS.**
After reviving A with plain `./uml` (new session
`uml-viewer-5f26e88b-bcc99ab2`, claude pid 14640), a
`:quit-for-restart` mail made the old viewer JVM (14466) save
`session.edn` and exit in <6 s. `./uml --restart` (via the public wrapper
only) launched JVM 15691 which **bound the same companion**: herdr session
name unchanged, claude 14640 the same process (etime continuity), no new
session in `herdr session list`, `companion.edn` `:session` unchanged.
Repeated twice more (JVM 16516, then 17316) with identical continuity —
three bind cycles total (`39/viewer-log.txt`, `39/final-state.txt`,
`39/session-roundtrips.txt`).

**Criterion 2 — namespace-tree and named-proposal view states each restored
— PASS.** Round-trip proof (viewer re-saves `session.edn` via the
production `save-session!` on each `:quit-for-restart`; restored state
survives a live session and appears in the re-save):
- Namespace tree + `:discussion-context {:context :real}` (set via
  production to-viewer `:context` mail): restored and retained;
  `:mail-seen` 14→15 with no mail replay (mailbox cursor preserved);
  `:proposal-id nil`, `:proposal false`.
- Named proposal view: the GUI click that sets `:proposal-id` is
  unreachable under the locked screen, so the saved file was augmented with
  the production writer `mailbox/write-session!` (the same function
  `save-session!` uses) adding `:proposal-id :p-1791033119418` and the
  proposal `:discussion-context`. After `--restart` the live JVM
  re-validated the id against the EDN's `:proposals` (unknown ids are
  dropped by `known-proposal-id`), kept it, flipped `:proposal` false→true
  and re-saved both the proposal view and context
  (`39/session-roundtrips.txt`). Explicit-path override: covered by
  restart-state priority (explicit > saved > legacy policy); saved-path arm
  exercised three times (log prints `Watching nil` for the nil explicit arg
  — that line echoes the CLI arg, not the restored path; restore happens in
  `restored-state`).

**Criterion 3 — after restore+restart combo, five functions all work —
PASS** (on JVM 17316, restored into the proposal view):
1. `:context` — to-agent mail 15 `{:context :real}` consumed
   (next-id 15→16), companion wake acked.
2. FIFO mail — to-viewer `:display` ids 17/18 both drained oldest-first
   (next-id 19, queue empty) within 8 s.
3. display — companion `:regen` (mail 16) → IR exit 0 → `:display` id 19
   consumed (next-id 20). Full production loop.
4. mtime reload — `./uml ir` from the project root, exit 0, diagram mtime
   1792025076→1792025169; viewer healthy afterwards, no reload exception in
   the log. (First attempt ran `./uml ir` from the wrong cwd — exit 126,
   no effect; corrected and rerun. Harness misstep only.)
5. metrics overlay — overlay join runs on every reload/display over the
   existing `.metrics` snapshots; viewer healthy, no errors. Visual
   verification of overlay colors is impossible under the locked screen;
   evidence is state-file/log level (noted).

**Wrapper refusal battery (criterion: refuse missing/changed identity
instead of replacing) — PASS**, on temp roots under `/tmp/uml-acc-39/`
(symlinked tool checkout; zero pollution; `39/r1.out`–`r6.out`):
- missing companion record → preflight: `Companion ownership is missing;
  cannot restart` (r1; also reproduced on the **real project A** while its
  `companion.edn` was `{}` — exit 1, no launch, no session,
  `39/a-restart-refusal.out`);
- cross-project record (B's `companion.edn` copied verbatim) →
  `Companion ownership is incomplete, provisional or belongs to another
  project; file retained`;
- launcher record live pid + wrong start → `Launcher process identity
  changed; cannot safely restart.` (r4);
- launcher dead + no JVM record → `No viewer JVM process record; cannot
  safely restart without authoritative JVM identity.` (r5);
- JVM record live pid + wrong start → `Viewer JVM process identity
  changed; cannot safely restart.` (r6).

**Final state:** A healthy (JVM 17316, claude 14640, session
`uml-viewer-5f26e88b-bcc99ab2`, recovery `:phase :running`, queues
drained); B untouched (viewer 19487, recovery `:running`, companion/
recovery/to-viewer md5 identical to pre-#39). Recurring known issue: the
"terminal open failed" osascript error on each JVM start (Terminal.app
automation under the locked screen; companion retained every time) —
pre-existing since #34, not a #39 regression.

### #40 Quality dispatch serial discipline — PASS (executed 2026-10-15 on project A, session uml-viewer-5f26e88b-06582e8d)

Method: one FIFO batch injected into A's to-agent.edn via the production path
(`mailbox/write-command!` ×4 + `sketch/notify-agent!` wake), targeting class
`:domain.mailbox` (`:ns "uml-viewer.domain.mailbox"`, kind :class):
`[7 :refresh-mutate, 8 :refresh-crap, 9 :refresh-mutate-all, 10 :regen]`.
A 2 s sampler (`40/serial-samples.log`) recorded quality-result.json
lifecycle, tool-process table and both mailboxes throughout. Known baseline:
issue #47 — 9 pre-existing herdr/recovery spec failures on macOS make
full-suite quality exits non-zero; per instruction this is baseline, not a
#40 failure. Contract pins (`uml-quality.json`): crap valid exit [0], mutate
[0, 3].

**Criterion 1 — per-operation version-1 record validation — PASS.**
- op 7 mutate (differential): record `{version 1, operation "mutate",
  projectRoot <canonical A>, exitCode 0, validResult true, tool
  "clojure-alias:mutate"}` published 01:56:28 (`40/mut7.log`: no changes
  since the 2026-10-12 successful run — legitimate no-op differential, exit
  0 ∈ [0,3]) → gate passed → IR ran → display (see deviation note).
- op 8 crap: record `{operation "crap", exitCode 1, validResult false,
  crapScope "targets"}` published 01:57:09 (`40/crap8.log`: full coverage
  table printed, "Coverage failed (exit 9)" inside; wrapper process exit 1,
  1 ∉ [0]) → gate REJECTED: no IR, no display; companion reported
  「CRAP 刷新被拒：记录显示 9 个 spec 失败 (#47)」.
- op 9 mutate-all: record `{operation "mutate", exitCode 2, validResult
  false}` published 01:57:59 (`40/mut9.log`: "Baseline: FAIL — specs do not
  pass without mutations. Aborting." — the #47 baseline failure; 2 ∉ [0,3])
  → gate REJECTED: no IR, no display.
- op 10 :regen (non-quality control): IR exit 0 → :display written and
  consumed — FIFO continued normally after two rejections.
The wrapper (`uml-command.ps1`) deletes any old quality-result.json before
each invocation and publishes atomically (tmp+move) after the real exit;
the companion additionally `rm -f`s the record itself before invoking
(visible in its pane commands). Records carry canonical projectRoot of A —
B was never written (`40/post-health.txt`: B mailboxes unchanged).

**Criterion 2 — serialization (no concurrent quality calls) — PASS.**
Per-sample tool-process counts show disjoint per-op windows: mutate
(pid 23539, ≤01:56:32), IR-7 (23610, 01:56:32), crap (23666/23823,
01:56:36–01:57:07), mutate-all (24158/24176/24335, 01:57:28–57), IR-10
(~01:58:03, sub-sample). Samples with 2–3 pids are same-op parent→child
(driver + spec runner), never two ops; op boundaries align with record
publish/delete transitions (publish → companion reads → delete → next op).
Companion popped ids 7→8→9→10 head-first with per-pop rewrites (TA queue
transitions in samples).

**Criterion 3 — IR exit 0 gates :display — PASS (both arms).**
Positive: op 7 (valid record + IR exit 0) and op 10 (regen IR exit 0) each
produced exactly one :display — to-viewer ids 8 and 9, both consumed
(`{:next-id 10, :queue []}`); diagram rewritten 01:58:03 with byte-identical
content (md5 a9ab83d6… unchanged — deterministic regen, no source/policy
change). Negative: rejected ops 8 and 9 produced no to-viewer writes at all
(TV stayed `next-id 8` across the 01:57:09 crap rejection and `next-id 9`
across the 01:57:59 mutate-all rejection).

**Deviation disclosed by the companion (accepted, flagged to team-lead):**
op 7's :display write was deferred until after op 8's CRAP run
(「顺序偏差」 in `40/companion-pane-final.txt`); the displayed EDN was the
one generated by op 7's IR before CRAP (no regen during CRAP), so gating was
preserved — only the display timing slipped. No concurrent quality calls
were involved.

**Snapshot hygiene:** `.metrics/` byte-identical to the pre-run backup
(`diff -rq` clean; mut7 no-op, mut9 aborted before writing, crap8 failed
before replacing its snapshot); quality-result.json left holding op 9's
record (exitCode 2, validResult false) — correct ownership, as :regen is
not a quality op. Screen remained locked (`screenIsLocked=1`), so window
evidence is CGWindowList enumeration — both "UML viewer" windows present
(A win 18492, B win 18494) — no screenshots taken. Final state: A and B
both healthy, recoveries `:phase :running` (`40/post-health.txt`).

### #41 `:omit` and `:context` (real / named proposal) flows — PASS (executed 2026-10-15 on project A, session uml-viewer-5f26e88b-06582e8d)

All mails injected through the **production entry point** — a throwaway JVM in
the runtime clone calling `adapters.sketch/request-agent!` (the same function
the right-click menu and `mail-context!` use), which does
`mailbox/write-command!` to `to-agent.edn` + `notify-agent!` (Herdr agent
prompt wake, `:woke? true` every time). Screen still locked
(`screenIsLocked=1`), so GUI-side evidence (inspector proposals list, **P**
key) is substituted with state files, mail flow and companion pane output —
noted per criterion. Evidence: `41/`.

**Criterion 1 — :omit → policy/proposal `:omit`, then IR, display only after
IR exit 0 — PASS (both arms).**

- Real diagram (mail 11, `:target {:id :domain.mailbox :kind :class ...}`, no
  `:proposal-id`): companion added `:omit [:domain.mailbox]` to
  `examples/uml-viewer.policy.edn` (diff vs `41/policy-pre.edn`: exactly one
  line added; `:proposals [{:id :p-1791033119418, :name "test", ...}]`
  preserved). Ordering: policy mtime 02:10:43 → diagram mtime 02:10:45
  (regenerated by `./uml ir`, header "Generated by clj -M:ir … Do not edit.",
  md5 a9ab83d6→9d2c77d4) → `:display` written to to-viewer (next-id 10→11)
  and consumed. EDN diff vs `41/diagram-pre.edn`: exactly one line,
  `:omit []` → `:omit [:domain.mailbox]`. Companion pane: "IR 退出码 0，之后
  已发 :display … :omit 只是带进 EDN、交给 viewer 在渲染时隐藏"
  (`41/phase-a-pane.txt`) — omit is IR data, viewer-side render filter
  (hierarchy.clj), so the class node correctly stays in the EDN.
- Named proposal (mail 12, `:target {:id :domain.policy ... :proposal-id
  :p-1791033119418}`): companion added `:omit [:domain.policy]` **inside that
  proposal's map**; top-level `:omit [:domain.mailbox]` untouched, proposal
  `:id`/`:name`/`:layers` preserved (policy lines 31–35,
  `41/phase-b-omit-proposal.txt`). Same ordering: policy 02:16:18 → IR
  02:16:21 → display id 11 consumed. Companion pane confirms both omits
  coexist (`41/phase-b-pane.txt`).
- `examples/uml-viewer.edn` never hand-edited: mtimes change only at IR
  moments and content diffs are exactly the two `:omit` lines.

**Criterion 2 — `:context :real` and named proposal both work; context pins
until the next `:context` — PASS.**

- Mail 13 `{:op :context :context :proposal :proposal-id :p-1791033119418
  :name "test"}`: consumed (TA next-id 13→14); companion: "现在讨论的对象切换
  到 proposal test（:p-1791033119418），直到下一条 :context 到来" and
  correctly notes a later proposal-less `:omit` still lands on policy
  (`41/phase-c1-pane-context-proposal.txt`). No IR/display — pure context
  switch, as specified.
- Mail 14 `{:op :context :context :real}`: consumed (next-id 14→15);
  companion: "现在讨论的对象切回真实命名空间树，直到下一条 :context 到来"
  (`41/phase-c2-pane-context-real.txt`).
- GUI substitution note: inspector proposal list and **P**-return are
  viewer-GUI interactions, unreachable under the locked screen. Data
  prerequisites verified instead: the generated EDN carries the `:proposals`
  block the inspector lists, `session.edn` shows the viewer on the real tree
  (no `:proposal-id`), and the mail/pane trail above covers the discussion-
  context contract. No screenshots taken.

**Final state:** A and B both healthy (viewers 16347/19487, both herdr
sessions running, both recoveries `:phase :running`, all queues drained). B
untouched — its policy md5 (95a86be5…) equals A's pre-run value
(`41/post-state.txt`). A's policy intentionally retains the two acceptance
`:omit` entries; diagram regenerated accordingly.

### #42 Legacy tmux companion.edn retained and refused with zero side effects — PASS

Executed 2026-10-14 in the main repo
(`/Users/billyq/Dev/unclebob/uml-viewer`, checkout c714f83,
branch `spec-32/macos-herdr-acceptance`). `./uml` chmod 755 first (mode
change left uncommitted per spec pre-fix decision; git status shows only
` M uml` throughout).

**Setup note.** The main repo had no tool checkout at
`.uml-viewer/uml-viewer/`, so the first `./uml` run never reached
ownership verification: pwsh reported
`The term '.../.uml-viewer/uml-viewer/scripts/uml-command.ps1' is not
recognized` — yet exited **0** (`uml.ps1` does `exit $LASTEXITCODE`, and
no native command had run, so `$LASTEXITCODE` is null → 0). That
missing-checkout-masks-as-success behaviour is recorded here as an
observation for #35's dependency-refusal scope; it is outside #42's
criteria. To make the ownership check reachable, a temporary symlink
`.uml-viewer/uml-viewer -> ..` (the repo is uml-viewer itself) was added,
and removed immediately after the run; the four mailbox files were never
touched by setup or teardown.

**Refusal evidence** (`./uml`, no arguments, stdout empty):

- Exit code: **1**
- stderr (verbatim):
  `uml: tmux/psmux companions are retired. Ownership and resources were
  retained; no takeover or cleanup was attempted. Finish the old
  viewer/session using the old release, verify that this project's legacy
  resources have exited, then archive .uml-viewer/companion.edn and start
  ./uml fresh with Herdr. Do not use --restart to migrate; retain
  session.edn and mailbox queues.`
- Refusal point: ownership-verification preflight —
  `uml-viewer.adapters.companion/select-backend!` → `recorded-backend`
  (src/uml_viewer/adapters/companion.clj:57) throws `legacy-guidance`
  because the record has no `:backend` field. This runs inside the
  lightweight preflight JVM invoked by `uml-command.ps1` before any
  launch manifest is written, before Herdr selection, and before the
  viewer JVM exists.

**Zero-side-effect evidence** (post-run, after symlink removal):

| Check | Before | After | Result |
|---|---|---|---|
| companion.edn SHA-256 | 488351c1…b986 | 488351c1…b986 | identical to snapshot |
| session.edn SHA-256 | 2195250b…b877 | 2195250b…b877 | identical to snapshot |
| to-agent.edn SHA-256 | e6481c38…dded | e6481c38…dded | identical to snapshot |
| to-viewer.edn SHA-256 | b36fb3bc…ceae4 | b36fb3bc…ceae4 | identical to snapshot |
| `.uml-viewer/` entries | 4 files | same 4 files | no launch-*.json, no new files |
| `uml-viewer-log.txt` | absent | absent | viewer never launched |
| `pgrep -fl java` | none (exit 1) | none (exit 1) | no JVM spawned |
| `herdr session list` | `default` stopped | `default` stopped | no Herdr session created |
| `git status --porcelain` | ` M uml` (chmod) | ` M uml` | no new changes |

Both acceptance criteria PASS: refusal message and exit code recorded
with the refusal point at ownership verification; pre/post diff of
`.uml-viewer/` is zero.

### #43 GUI close cleans only own resources; project B keeps working; ambiguous identity fails explicitly — PASS (executed 2026-10-15, A closed 02:26:50–51)

Screen still locked (`screenIsLocked=1`), so the window close event itself
could not be synthesized (as established in #38 phase C: AX exposes no close
button and CGEventPostToPid cannot hit window chrome under a locked screen).
Substitute: the **same production function** the red button invokes — a
throwaway JVM calling `adapters.sketch/shutdown-children!` on project A,
which is exactly what `on-main-close` → `exit-app!` runs (the in-process
`halt-vm!` tail is covered below). The windowClosing event itself was not
triggered — noted per ticket. Evidence: `43/`.

**Criterion 1 — close cleans this project's resources, no revival — PASS.**
`shutdown-children!` completed in 1.27 s (02:26:50.568→02:26:51.837) with no
error output. Afterwards (`43/a-post-close.txt`): all five companion-side
processes gone (herdr server 16364, pane shell 16394, runner 16481, claude
16494, recovery controller 16502); `recovery.edn` shows `:phase :cancelled`
with `:reason "Viewer closed; recovery revoked"` — recovery cancelled as part
of the chain, so the controller never relaunched the killed companion;
`herdr session list` lost `uml-viewer-5f26e88b-06582e8d` (only B and stopped
`default` remain); A's session socket directory deleted; `companion.edn`
rewritten to `{}`. No-revival watch: 8 polls over 16 s plus a final check at
+7 min — zero A pids, one uml-viewer session (B), session dir absent
(`43/no-revival-watch.txt`, `43/final-state.txt`). The viewer JVM itself was
then ended with SIGTERM (exit substitute for the in-process
`halt-vm!`/`System/exit 0`; verified in #38 that the viewer JVM installs no
shutdown hooks, so signal vs. in-process exit has identical external
effects); gone in 1 s. A left fully closed with saved `session.edn` intact
for #39 (`43/final-state.txt`).

**Criterion 2 — project B unaffected — PASS.** Across A's close: B's six
processes (viewer 19487, server 19507, shell 19533, runner 19630, claude
19634, controller 19644) kept identical lstart timestamps; `companion.edn` /
`recovery.edn` / `to-viewer.edn` md5-identical; pane content byte-identical
(`43/pre-state.txt` vs `43/b-survival-static.txt`). Functional proof: a
harmless `:context :real` mail injected into B afterwards was consumed
(B-TA next-id 1→2, queue empty) and acknowledged on B's pane
(`43/b-pane-after-wake.txt`); B recovery still `:phase :running`.

**Criterion 3 — ambiguous identity fails explicitly — PASS.** After the
close, `companion.edn` is `{}` (ownership gone); re-running the production
`shutdown-children!` on A prints exactly
`UML viewer: companion cleanup: Companion ownership is missing, ambiguous or
belongs to another project` and changes nothing — A companion.edn still `{}`,
B files md5-identical, session inventory unchanged
(`43/ambiguous-identity-probe.txt`). Complements #42 (legacy tmux record
refused with zero side effects) and the herdr specs covering identity-change
refusal.

### #44 Acceptance log consolidation and status landing — DONE (PASS path)

This file is the single consolidated acceptance log (spec #32 user
story 17): complete timeline, environment record, per-criterion one-line
verdicts with evidence pointers, and key commands with exit codes for
everything executed. Final status **PASS** per the table above:

- All executable criteria #33–#43 passed (see per-ticket sections; #34
  documents the FAIL→fix→PASS history with defect #45, fixed and closed).
- Terminal attach marked NOT VERIFIED and handed to follow-up issue
  **#49** (user story 18, Windows #30 precedent; supersedes #46).
- Observations and follow-ups consolidated in the 问题清单 section below
  (#47, #48, #40 timing observation, and smaller items).
- Tracker operations (status comments, #44 closure) are performed by the
  team lead.

### Terminal attach (platform attach, manual criterion) — NOT VERIFIED (handoff → #49)

Per spec #32 Out of Scope and Windows precedent (#21 → #30): no real
manual terminal-attach observation was performed under AFK execution with
a locked screen. Marked NOT VERIFIED and handed to follow-up issue
**#49**, which also carries the related Terminal AppleScript failure
observed throughout this acceptance (`terminal open failed … -10006`
under the locked screen; the companion is retained each time and the
fallback `herdr session attach` command is printed — see #39 section).
Supersedes the earlier handoff issue #46.

## 问题清单 (problem list — PASS path follow-ups)

Consolidated by #44, 2026-10-15. Acceptance PASS; the items below are
observations and follow-ups for maintainer adjudication, none blocking.

1. **#47 — 9 pre-existing herdr/recovery spec failures on macOS (open).**
   `clojure -M:crap` coverage exits 9 (wrapper process exit 1) and
   mutation runs abort with "Baseline: FAIL — specs do not pass without
   mutations" (exit 2). This is the known baseline during #40: quality
   gates produced no fresh snapshot, and the dispatch correctly rejected
   the invalid records. The companion noted (unconfirmed) that the 9
   failures may be environmentally influenced. Evidence: `40/crap8.log`,
   `40/mut9.log`, `40/quality-result-final.json`.

2. **#48 — ⌘Q bypasses companion cleanup (open).** The viewer installs no
   QuitHandler and no shutdown hook: quitting via ⌘Q (or dock) terminates
   the JVM without running `shutdown-children!`, orphaning the herdr
   session and leaving recovery armed (zombie relaunch risk). Only the
   red button (`on-main-close`) runs cleanup. Found during #38 (close-path
   analysis); GUI verification of the red-button path itself was done via
   the production `shutdown-children!` substitute under the locked screen
   (#38 phase C, #43).

3. **#40 display-timing observation (accepted with observation).** In the
   #40 FIFO batch, op 7's `:display` was deferred until after op 8's CRAP
   finished; the deferred write carried the EDN generated (and gated)
   before op 8, so gate integrity held and exactly one display occurred.
   Team-lead adjudicated: PASS with observation, recorded here; not a
   separate issue. Evidence: `40/serial-samples.log`,
   `40/companion-pane-final.txt`.

4. **probe-herdr renders unparseable versions as "(unknown: …)".** In
   candidate-rejection messages the version slot falls back to the
   literal `unknown` (`adapters/companion.clj:130`,
   `(or (:release %) "unknown")`), so a herdr whose `--version` output
   does not match `herdr X.Y.Z` is reported as "(unknown: not a herdr
   X.Y.Z >= 0.9.3)" — the real version string is not echoed, which
   hampers diagnosis. Display-level observation; selection logic itself
   is correct (verified in #34 control runs and every launch preflight).

5. **Wrapper: missing tool checkout reports an error yet exits 0.** When
   `.uml-viewer/uml-viewer/` is absent, pwsh reports
   `The term '.../.uml-viewer/uml-viewer/scripts/uml-command.ps1' is not
   recognized` but `./uml` still exits 0, because `uml.ps1` does
   `exit $LASTEXITCODE` and no native command had run (`$LASTEXITCODE`
   is null → 0). A missing-runtime failure thus masquerades as success
   in exit-code terms. Recorded in the #42 setup note.

6. **Terminal AppleScript fails under the locked screen (-10006) → #49.**
   Every viewer JVM start logs
   `terminal open failed: …不能将"title displays settings name of tab 1 of window id N"设置为"false"。(-10006)`
   and retains the companion with the manual
   `herdr session attach '<session>'` fallback printed. Pre-existing since
   #34's first run; folded into the terminal-attach handoff #49.
   Evidence: `39/viewer-log.txt` (3 occurrences), #34 watch logs.

7. **Host sleep (~6 h) during #39 — no effect.** The host slept
   02:47→08:37 mid-launch of the fresh-start JVM; the launcher resumed
   and completed normally after wake. Herdr sessions, companions and
   recovery controllers on both projects were unaffected. Noted for
   timestamp interpretation in `39/` evidence.

Resolved during acceptance (kept for history): **#45** — Herdr server
died on the first `workspace create` (`server_unavailable`, macOS arm64);
fixed by 0b6f1ea (session-name sun_path), with follow-up fixes 6355aa4
(login shells) and 5e4407e (canonical-mode retry); #34 re-run 3 passed and
#45 is closed. Also recorded: **ADR 0004 observation** — PATH-prepending
cannot select a *lower* Herdr (highest compatible release wins), so "put
an older Herdr first on PATH" is not a downgrade workaround; only an
exclusive-PATH shim selects an older release (#34 timeline 22:47:26 row;
`34-failure/repro-2/`, `34-uml-invocation-0.9.3.txt`).
