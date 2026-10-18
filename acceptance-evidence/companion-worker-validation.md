# Non-blocking companion startup validation

Local validation on native Windows, LOVE 11.5 and Herdr 0.9.5. Changes are uncommitted.

## Change

The viewer starts a separate headless LOVE companion worker through the native Windows process boundary. Startup, wake and shutdown execute in that worker. Generation-scoped command/result files and exact process creation identity protect result consumption. The viewer continues its event loop while the worker initializes, fails or cleans up. Restart waits for the old worker to confirm keep and exit before the viewer exits.

The mouse-move callback now forwards coordinates to cimgui-love. The waiting scene uses tagged EDN maps so it can render before display arrives. Native process declarations share the FILETIME type and record the identity from the original process handle.

Cancellation is cooperative at lifecycle stage boundaries. Startup exceeding 120 seconds reports cancellation pending and retains ownership. A permanently blocked underlying call is not forcibly terminated. Ready means connected; fresh startup still waits for display. A restart requested during incomplete fresh initialization may be refused with ownership retained; the viewer continues responding but does not release a replacement startup over the partial resources.

## Verified

- `lua scripts/run_busted.lua love/spec`: 511 successes, zero failures/errors, final run 3.5 seconds.
- Companion worker tests: 16 successes, including stale generation rejection, PID reuse, unknown exit, startup cancellation, lock contention, timeout and late results.
- Native mouse callback regression failed before MouseMoved forwarding and passed afterward. Current native detail-scroll test passes; screenshot confirms hovered op8, and the driver verifies scroll and proposal controls. Fixture: `%TEMP%/uml-detail-scroll-ea41fd832a9949c184fcc19eb1713842`.
- Waiting-scene permanent regression failed against an isolated in-memory copy of the old plain-table scene and passed against the current tagged scene. Fixture: `%TEMP%/uml-mail-redgreen-20261018-c8f31a/run.lua` (`red` exits 1, `green` exits 0).
- Real Herdr lifecycle fixture: startup stages reach ready, waiting canvas renders, context mail is consumed FIFO, and normal close confirms empty ownership. Fixture: `%TEMP%/uml-worker-8a0a8202581448d780be58eeceb94218`.
- Failure fixture: invalid ownership is refused, UI continues processing frames and exits normally; ownership bytes unchanged. Fixture: `%TEMP%/uml-worker-failure-3c8533ea6ffd4449b258a390870626b8`.
- Early-close fixture: frame-4 close is processed; the helper cancels at the server stage before creating Herdr and retains provisional ownership. Corrected fixture assertion exits 0: `%TEMP%/uml-worker-cancel-d3c2172f6eb145589ec3f1198360cf2a`. The earlier fixture expected no ownership and exited 1; that incorrect test assertion is not counted as a pass.
- Native spawn test: same LuaJIT VM launches twice; Unicode cwd and literal argv including empty arguments round-trip; immediately exiting child retains exact creation identity. The one-off spec was not kept in the repository. Fixture `%TEMP%/uml-native-spawn-6e58439270bb40768260f80cae594d77`. A later probe loaded `process` and `process_identity` in both orders in one VM; `owner_native` and `spawn_native` passed only with the shared `UML_FILETIME` declaration.
- Isolated public fresh/restart/close: old helper keep/exit and lock release confirmed; saved path, proposal discussion context, cursor and default view restored; ownership and recovery journal hashes unchanged; final resources released. Fixture `%TEMP%/uml-public-restart-7f3c92/validation-result.txt`. This uses ping.exe as a fake Claude child. Recovery had reached terminal failed; normal active supervision and custom camera/selection/detail were not validated by this run.
- Nine runtime source files deployed into the local ignored installed checkout, all raw SHA256 hashes equal to source.
- `git diff --check` passes.

## Quality and root-project limitations

CRAP and the first differential mutation failed in LuaCov coverage conversion (`reporter.lua:135`, nil file attributes). A read-only path scan identified an inaccessible deeply nested installation beneath an old `.claude/worktrees` directory. Tests pass, but the quality records have `validResult: false`. No worktrees were deleted and no failed mutation was retried. At the user's request, the serial quality runner and its owned descendants were stopped before repairing the quality pipeline. Four mutation requests completed with exit 1 / invalid results: companion_lifecycle, herdr, process and process_identity. The viewer request was interrupted without a final result; companion_worker and document were not run. Those invalid outputs were discarded after the pipeline repair. Do not infer a successful quality gate from functional tests.

The quality pipeline repair has now passed its public CRAP check: 511 tests, exit 0, and a fresh version-1 whole-tree record with validResult true. crapper is committed and pushed to origin/main as 7d733c3cf00ad786988de31cab2787e3a104fdb1, including a README scope correction, and its worktree is clean. cluacov 1.0.0-1 was installed for Lua 5.4; the 16 native-hook branches executed, with 42 related tests passing and no skips. Mutator commit/push and its CI verification are tracked separately in the delivery ledger.

After the repair, `love/uml/adapters/viewer.lua` also completed differential mutation: 511 tests, actual exit 0, a fresh version-1 record with validResult true, and a published snapshot. The uncovered mutants are retained as coverage gaps; that file was not rerun and mutation was not forced to all. IR generation then exited 0 and wrote `examples/uml-viewer.edn`.

The root project's public restart was also attempted and refused after waiting for its existing launcher (PID 7040). That process was not killed, its identity was not replaced, and the companion was not replaced. The root-project viewer has not been successfully relaunched by this change.

## Recovery controller launch and visible attach

A fresh launch stalled after the recovery stage. `Process.spawn_detached` read the PowerShell launcher through `io.popen`; the long-lived recovery controller inherited the pipe, so the read did not reach EOF until the controller exited. Isolated probe: ping.exe held for its full 30 seconds and `spawn_detached` returned after 29,875 ms. The adapter now runs the launcher with `os.execute` and reads its result from a unique launch file; the same probe returned in 1,046 ms while the child kept running. Regression added to `scripts/native_process_spec.lua`; `scripts/native_test_guard.lua` scopes the new `os.execute` form.

`CompanionLifecycle.open_window` existed and was tested but was never called after the LOVE rewrite, so Windows never opened the documented `herdr session attach` window. The helper now opens it once for a fresh companion, before publishing ready, and reports the manual attach command in the ready message when the terminal does not open. Restart does not open a second window. `UML_COMPANION_NO_ATTACH=1` disables this for automated fixtures.

The wrapper runs the ignored installed toolchain under `.uml-viewer/uml-viewer/`, so the first relaunch still executed old code until the changed `love/` and `scripts/` files were copied there.

Verified after the fixes: `lua scripts/run_busted.lua love/spec` 515 successes; `lua scripts/run_native_busted.lua` 10 successes. A fresh public launch published the pane and reached `recovery → committed → ready` in four seconds; the attach client (PID 16632) opened in Windows Terminal with title `LZX-QYJ: uml-viewer`, and the pane title showed the companion agent working. The earlier stuck run was stopped through its own protocol: a stop command plus a recovery-control cancellation, which let the controller exit, the blocked read finish, and verified cleanup release the workspace, server and session (event `:stopped`, ownership `{}`). No process was force-killed.

## Mutation worker isolation

Differential mutation of `companion_worker.lua` repeatedly ended as "Baseline failed" (exit 2). Mutation workers omit generated trees (`.metrics` and similar) and reject any file a test writes inside the worker. Three spec dependencies on worker state were fixed: `roundtrip_spec` skips absent roots, the real lua-fixture test in `metrics_spec` runs only when its snapshots exist, and the `companion_worker_spec` fixture journal base now has a missing parent directory. Mutants that bypass the injected `write` now fail instead of creating `journal.event-*.edn` files in the worker. The leaked keys were identified with a temporary diagnostic in the installed mutator copy, which has since been reverted. Pure tests: 516 successes.

Final quality flow, serialized, with `quality-result.json` removed before each call:
- `./uml mutate love/uml/adapters/process.lua`: exit 3, validResult true (15 killed, 5 survived in `M.win_quote`, 173 uncovered).
- `./uml mutate love/uml/adapters/companion_worker.lua`: exit 3, validResult true (77 killed, 18 survived, 16 uncovered, 81.1%).
- `./uml mutate love/main.lua`: refused by the wrapper as outside the business scope; the tool was not started.
- `./uml crap`: exit 0, validResult true, whole-tree.
- `./uml ir`: exit 0.

Survivors and uncovered mutants are kept as findings, with no reruns and no `--mutate-all`.

## Headless helper windows

During acceptance, a second "Untitled" LOVE window appeared and stopped responding. It belonged to `companion-runner`. `conf.lua` disabled the window only for `ir` and `companion-worker`, but every helper mode replaces `love.run` with no event loop. All helper modes (`companion-runner`, `companion-controller`, `companion-smoke`, `companion-preflight`) now start without a window. Verified: pure tests 517 successes, native tests 10 successes, and a standalone runner probe opened no window. The old stack was closed normally through the viewer (`:stopped`, ownership `{}`). In the fresh launch, the viewer is the only LOVE window and is responding, and the companion reached ready. Quality checks: `./uml crap` exit 0, validResult true. `./uml mutate love/conf.lua` was refused as outside the business scope. `./uml ir` exit 0. The user accepted the launch.
