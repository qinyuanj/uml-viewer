# UML viewer

The diagram is already on screen.

- Do **not** edit `examples/uml-viewer.edn`. It is generated from
  `examples/uml-viewer.policy.edn` by the LOVE IR pipeline via `./uml ir`.
- Policy follows **namespace nesting**, not invented layers/components.
  Dots after the prefix are the tree. Do not add Domain/Engine-style
  packages. **Layer** and **component** mean the same thing.
- TypeScript, Rust, Python, and Lua are the supported scanners. This
  repository's policy is `:lang :lua` with `:src "love"`. A class with no
  CRAP or mutation data is red; never hide it. The overlay joins a
  snapshot by the class `:ns`, then by the class id. crapper and mutator
  name a module by its project-relative path (`love/uml/a/b.lua` is
  `love.uml.a.b`); `:prefix "love"` keeps the scanner `:ns` identical, so do
  not strip source roots on either side. A non-Lua project sets `:lang` or
  `:sources`.
- `:proposals` are named groupings that are **not** in the source. Preserve
  them when rewriting policy. Do not invent them on launch. The inspector
  lists them; **P** returns to the namespace tree. If instructed, add a
  named proposal (default name is a timestamp) and regenerate the IR.
- After source or policy change: `./uml crap`, `./uml mutate` on changed
  source files (differential), then `./uml ir`. Uncovered mutants are
  coverage gaps: keep the snapshot; do not re-run the file or force a
  full mutation because mutate exited non-zero.
- Mail files are queues `{:next-id n :queue [cmd …]}`. Pop the head of
  `:queue` as you handle it (rewrite the file). Oldest first.
- Right-click quality mail uses public `./uml crap` / `./uml mutate` dispatch
  (install the entry with `get-uml-viewer --install-only` if absent).
  `:refresh-crap` follows declared scope (standard tools measure the whole
  business tree; an operation override may declare target scope);
  `:refresh-mutate` is differential on target files; `:refresh-mutate-all`
  passes `--mutate-all` only on those files. Invoke mutation once per source
  file, never a file list. Scoped mutation keeps explicit targets, without
  source-root flags.
- Serialize quality calls. Remove the old `.uml-viewer/quality-result.json`
  before each call, await its actual exit, and require a freshly published
  version-1 record matching operation, canonical project root and actual exit,
  with `validResult: true`. Only then generate IR; only IR exit 0 permits
  `:display`. Missing/stale/mismatched records or failures stop that request's
  IR/display and report the cause; continue later FIFO mail. Keep findings
  snapshots without retries. Exit 3 requires a tool or project operation
  declaration; there is no Clojure alias route.
- `:omit` → add `:id` to the current proposal's `:omit`, or to policy `:omit`
  on the real diagram, then IR; display only after IR succeeds.
- `:context` mail: `{:context :real}` or `{:context :proposal :proposal-id
  id :name "..."}` is the diagram under discussion. Stay on that context
  until a later `:context` arrives.
- Prefer `./uml` (fresh start: spawn this companion and LOVE viewer, wait for
  `:display`) and `./uml --restart` (new LOVE process, keep this session,
  restore last view). If `./uml` is missing, tell the user to run
  `get-uml-viewer` in this directory (it clones uml-viewer into gitignored
  `.uml-viewer/uml-viewer/`).
- Do not start the viewer on launch; EDN reloads when the file mtime
  changes. To restart it: write `:quit-for-restart` to
  `.uml-viewer/to-viewer.edn`, wait for the LOVE viewer to exit, then
  `./uml --restart`. The new process restores the saved diagram path,
  discussion context, view, and mailbox cursor; omitting the path uses the
  saved diagram, while an explicit path overrides it. The public wrapper
  verifies and waits for the old launcher/LOVE identity and refuses missing
  or changed companion ownership instead of replacing it. Do not pass
  `--restart` except through that wrapper. Do not SIGKILL. Companion
  ownership in `.uml-viewer/companion.edn` must explicitly record Herdr;
  closing the viewer cancels recovery first and releases only this
  project's verified resources. Verified abnormal exits retry at most three
  times (1, 2, 4 seconds) through the durable controller; normal or unknown
  exits stop recovery. Other Claude sessions stay up.
- Herdr is the sole companion backend on every supported platform.
  Delivery targets are **WSL2 + Ubuntu 24.04 + WSLg** and **macOS Apple
  Silicon**; Windows native and Intel Mac are unsupported (ADR 0007).
  Legacy tmux/psmux or unmarked ownership is retained and refused; before
  migration read `docs/herdr-backend-contract.md` §9.
- Installer prerequisites are LOVE 11.5, Lua (5.1/5.4) with lfs and
  luacov, git, Herdr >= 0.9.3, and Python 3.11+. No PowerShell is
  required. `get-uml-viewer` provisions pinned
  cimgui-love 1.90-1 bindings and DLL automatically into `.uml-viewer/`.
  It always fetches all three quality tools — crapper, mutator and dryer —
  and provisions their project-local `.uml-viewer/<tool>/.venv` environments.
  dryer reports duplicate structure in `.metrics/dry.edn`; nothing overlays
  that snapshot, so `./uml dry` is read from the terminal.

Do not commit or push unless asked. Esc interrupts a turn in this terminal;
do not kill the process on interrupt.

## Agent skills

### Issue tracker

Issues 和 specs 使用 GitHub Issues；操作前读取 `docs/agents/issue-tracker.md`。

### Triage labels

使用五个默认 triage 标签；分类和应用标签前读取 `docs/agents/triage-labels.md`。

### Domain docs

领域文档采用 single-context；探索代码前读取 `docs/agents/domain.md`。
