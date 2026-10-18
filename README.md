# UML viewer
## Abstract

This project displays a project as UML. The UML is *dynamic*. You can click
elements to drill down — all the way to source code. You can also play
what-if games by proposing architecture changes.

The app is coupled to an agent assistant. The two communicate, so you can
display the UML, drill down, tell the agent what you don't like, and watch
it make the changes. Describe a proposal and the agent will build a new
diagram. Keep revising that proposal until you like it, then tell the
agent to make the code match.

Color coding comes from CRAP and mutation metrics. You can adjust the
thresholds in config if you like.

The viewer and agent treat namespaces as components. Modules inside those
namespaces are the components' elements. Nesting can be arbitrarily deep.
You build an architecture by moving modules into the right namespaces with
the right dependencies.

You choose architectural levels by telling the agent what coupling and
cohesion criteria to use. For example, I tell my agent to group modules
into components by the Common Closure Principle.

Dependencies that point from a higher level to a lower level are colored
red, in accordance with the Dependency Rule of Clean Architecture.

The bottom line: you can view the structure of your code, see how well
tests cover it, play what-if games, and manipulate the system from a high
level.

## Tech

A [LOVE 11.5](https://love2d.org/) app using
[cimgui-love](https://codeberg.org/apicici/cimgui-love) that lays out and
draws UML from an EDN IR. A **policy** plus a language-specific scanner
writes the topology; this tool displays it, routes the arrows, colors CRAP,
and lets you click. The JVM implementation and Clojure scanning were removed
in the LOVE cutover; this release supports native Windows only, and macOS is
unsupported this round.

The IR is **topology**: the namespace tree, classes, and edges. **Metrics**
(CC, coverage, CRAP, killed/survived/uncovered) come from `.metrics/`
snapshots produced by [crapper](https://github.com/qinyuanj/crapper) and
[mutator](https://github.com/qinyuanj/mutator). The viewer overlays
those files at load, keyed by namespace and function name. Agents edit the
policy, not the IR. See [Policy](#policy).

## Run

Needs LOVE 11.5, PowerShell 7, git, Herdr >= 0.9.3, and Python 3.11+.
The installer provisions pinned cimgui-love 1.90-1 Windows x64 bindings and
DLL automatically. There is no Java or Clojure dependency.

From **this repo** (the viewer and IR generator are the `love/` tree):

```powershell
& 'C:\Program Files\LOVE\lovec.exe' .\love ir .\examples\uml-viewer.policy.edn
& 'C:\Program Files\LOVE\love.exe' .\love .\examples\uml-viewer.edn
scripts\test_lua.ps1                 # dual-interpreter busted suite
```

From **any project** you want to view (no local uml-viewer checkout
needed):

```bash
cd /path/to/the-project
curl -fsSL https://raw.githubusercontent.com/qinyuanj/uml-viewer/main/scripts/get-uml-viewer -o get-uml-viewer
chmod +x get-uml-viewer
./get-uml-viewer               # fetch, write ./uml, start
./uml                          # later fresh starts (returns immediately)
./uml --restart                # companion only: new LOVE process, restore last view
./uml ir                       # policy → IR
./uml crap                     # .metrics/crap.edn for the declared tree
./uml mutate love/uml/a/b.lua  # differential mutation of these files
```

`scripts/get-uml-viewer` is in this repo. It clones uml-viewer into
**gitignored** `.uml-viewer/uml-viewer/` (a nested `.git` there is invisible
to the project's repo), writes `./uml`, and starts the viewer. `--install-only`
skips the start. `UML_VIEWER_REPO_URL` and `UML_VIEWER_REF` override the clone
source (default `qinyuanj/uml-viewer`, branch `main`). Installation and analysis
use **PowerShell 7** (`pwsh`), including on Unix. Install it yourself from
https://aka.ms/powershell-install; Windows PowerShell 5.1 is unsupported.
git, LOVE, Herdr, and Python 3.11+ are also user-installed prerequisites.
Install LOVE from https://love2d.org/. The installer downloads and verifies
cimgui-love into `.uml-viewer/cimgui-love-1.90-1`; public entries use that
project-local dependency automatically. `UML_CIMGUI_DIRECTORY` can override
it with an existing matching binding and DLL. Set `UML_LOVE_EXECUTABLE` when
`love` is not on PATH.
Install Herdr >= 0.9.3 from
https://github.com/herdrdev/herdr/releases. The installer preserves existing
policies and proposals and does not create a policy or generate a diagram
during `--install-only`.

For native Windows, download `scripts/get-uml-viewer.ps1` from the same ref,
then run from the project:

```powershell
pwsh -NoProfile -File .\get-uml-viewer.ps1 --install-only
.\uml.ps1 ir
.\uml.ps1 crap
.\uml.ps1 mutate 'src/file with spaces.lua'
```

The installer writes `uml` and `uml.ps1`; both forward to the installed
PowerShell runtime, and Windows uses `.\uml.ps1` (or `pwsh -File .\uml.ps1 ...`)
directly. No `.cmd` shim is installed: a batch wrapper hands arguments to
`cmd.exe`, which re-quotes them and cannot carry literal shell characters.
Viewer startup also uses PowerShell 7: a detached launcher starts the LOVE
process with individual arguments and writes its output to the project log.
Fresh startup uses **Herdr as the sole companion backend** on native Windows.
This release does **not** support macOS; historical macOS acceptance evidence
remains historical only. `--backend herdr` remains accepted; `--backend tmux`
and `--backend psmux` are rejected before creating resources. Restart, wake and
close require explicit Herdr ownership and verify its recorded identities.
Legacy or unmarked ownership is retained and refused; see
[legacy migration](docs/herdr-backend-contract.md#9-旧后端退役与迁移).
Startup checks the preinstalled CLI is **>= 0.9.3** and the bundled protocol is **22**
without starting a server, terminal, companion or detached Viewer. With several
`herdr` executables on `PATH`, the highest compatible release is used for every
Herdr call ([ADR 0004](docs/adr/0004-herdr-executable-selection.md)). Install from
[the official release](https://github.com/herdrdev/herdr/releases)
or [herdr.dev](https://herdr.dev), then verify the version.
There is no download, upgrade or fallback. `--backend herdr` creates an isolated
named session and records its server, shell, pane and process creation identities.
Windows opens the full `herdr session attach` client in Windows Terminal;
`herdr terminal attach` is not supported on Windows. The attach child clears
`HERDR_ENV` so launching from an existing Herdr pane does not trigger its nesting guard.
If the terminal cannot open, the companion and its ownership remain intact and
the log prints an attach command to run in a separate terminal. Do not start a
second companion to retry attachment. Herdr starts a detached project recovery
controller. A verified abnormal Claude exit is retried at most three times,
after 1, 2 and 4 seconds; the count resets only after 60 seconds of verified
continuous child uptime. A normal exit, unknown exit evidence, or lost pane
ownership stops recovery and retains mail with a diagnostic. Closing the
diagram revokes recovery and waits for the controller to exit before removing
only this project's Herdr resources. Recovery needs direct native `claude.exe`
(or `node.exe` plus the official `cli.js`); shell adapters cannot prove the
child identity. Windows native acceptance is **partial**: real Claude recovery, namespace
restart continuity and project isolation were recorded, with seven
remaining native checks tracked by the
[Windows Herdr #30 record](docs/windows-herdr-30-observations.md)
([baseline](docs/windows-herdr-30-baseline.md)).
Linux/WSL companion launch is unsupported after tmux retirement. Historical
backend evidence remains in [platform acceptance](docs/platform-acceptance.md).
A standalone downloaded
Unix installer fetches its PS1 sibling from `UML_VIEWER_REF`; for custom hosts,
put both files together or set `UML_VIEWER_INSTALLER_URL` explicitly.

Native Windows requires a native `love.exe` (or a PowerShell `.ps1` entry)
and the cimgui-love dynamic libraries. The wrapper rejects batch
`.cmd`/`.bat` wrappers because they cannot preserve arguments. There is no
JVM or Clojure CLI requirement; `viewer-jvm.json` is no longer recognized.

`CLAUDE_BIN` continues to select the companion program. On Windows use the
[native Claude installation](https://code.claude.com/docs/en/setup) (`claude.exe`)
or a `.ps1` adapter. Claude `.cmd`/`.bat` wrappers are rejected because they cannot
preserve arbitrary prompts and argv. If retaining the npm package, point
`CLAUDE_BIN` to a PS1 adapter like the following, replacing the two paths with
your actual `node.exe` and the official installed package's `cli.js` paths:

```powershell
$PSNativeCommandArgumentPassing = 'Standard'
& 'C:\Program Files\nodejs\node.exe' 'C:\Users\you\AppData\Roaming\npm\node_modules\@anthropic-ai\claude-code\cli.js' @args
exit $LASTEXITCODE
```

This invokes Node with individual arguments; it does not forward through the npm
batch wrapper. Set `$env:CLAUDE_BIN = 'C:\path\Claude adapter.ps1'` before launching.
The adapter must preserve argv and the child exit status.

Every project gets
[crapper](https://github.com/qinyuanj/crapper) and
[mutator](https://github.com/qinyuanj/mutator) cloned side by side into
`.uml-viewer/` (branch `main`; override with `CRAPPER_REPO_URL`,
`CRAPPER_REF`, `MUTATOR_REPO_URL`, `MUTATOR_REF`). The installer always
fetches both tools, including in repositories that still contain a `deps.edn`
from the removed Clojure implementation. It then provisions each dispatcher
path: `.uml-viewer/<tool>/.venv` with the tool installed editable. Failure to
fetch either tool is fatal instead of a warning. They need Python 3.11+
(select the interpreter with `UML_PYTHON_EXECUTABLE` when needed). Re-run
`get-uml-viewer --install-only` to fetch updates; the editable installs use
the updated source. Native Windows entries run each tool as
`.venv/Scripts/python.exe -m crapper` (or `-m mutator`); sibling checkouts are
not on `PATH` and are not used by the dispatcher. The mutator `main` branch includes
argument-vector test commands and Windows process-tree timeout cleanup. For the
standalone mutator, WSL2 with Ubuntu remains its recommended Windows setup;
this recommendation does not apply to companion launch. Keep a WSL project in the
Linux home directory, rather than `/mnt/c`, and install Lua/coverage
prerequisites with mutator's `scripts/setup-ubuntu.sh`. Native Lua validation
is recorded in
[platform acceptance](docs/platform-acceptance.md#mutator-lua-接入复验--2026-10-10).
The local default-runner fix loads `busted.runner` directly in the selected Lua
interpreter; it does not require a native Busted executable or a custom
`--test-command`. Use it with the matching crapper `main` checkout for LuaRocks
module-path setup. It needs a real `lua5.4` or `lua` executable; interpreter
`.cmd` / `.bat` shims are resolved by crapper when possible. LuaRocks
installations also need their `bin` directory on PATH and their Lua module
directories in `LUA_PATH` / `LUA_CPATH`; a terminal
opened before these variables were set may need to be reopened. See
[mutator's setup and limitations](https://github.com/qinyuanj/mutator/tree/main#windows).

Analysis always runs from the examined project, forwards individual arguments,
and preserves the tool's exit status. Standard crapper/mutator invocation is the
primary quality route; a repository command override is an optional extension.
The installed standard tools accept CLI options rather than loading persistent
business-scope settings. `uml-quality.json` can persist these options through
`standardTools`, independently of optional executable overrides in `operations`:

```json
{"version":1,"standardTools":{
  "sourceRoots":["business tree","common"],
  "exclude":["business tree/generated","**/vendor"],
  "crapperOptions":[],
  "mutatorOptions":["--busted-task","fast","--max-workers","3"]
}}
```

Every field is an optional array of nonempty strings. Unknown fields fail.
`standardTools` is the primary quality route. There is no Clojure alias route
after the cutover; without a declaration the standard tools inspect the whole
project. Explicit `operations` take priority
and still require complete valid command declarations. Roots resolve from the
project root and must be existing directories inside it. Exclusions are anchored
at the project root, use either separator, and forbid absolute paths or `..`.
`*` and `?` match within one segment; `**` matches whole directory segments,
including zero segments; matching a directory excludes descendants. Matching is
case-sensitive. Tool option arrays cannot declare owned routing flags (`--root`,
`--source-root`, `-s`, `--exclude`, `--full-tree`, `--changed`, `--mutate-all`).
Scope fields own roots/exclusions; only explicit refresh arguments request full
mutation. Refresh arguments cannot override configured roots/exclusions.

With standard scope configured, mutation expands supplied files/directories (or
`--changed` Git additions/modifications/untracked files) and filters to business
sources before calling Mutator. Without targets it selects the configured trees.
An empty filtered selection fails explicitly without launching a tool. Missing
paths fail; scoped refresh does not accept textual path filters. Omitted targets
are diagnosed on stderr. Mutation retains differential behavior and snapshots.

Without persistent configuration, supply scope flags per invocation:

```powershell
.\uml.ps1 crap --source-root 'business tree' --source-root common --exclude 'business tree/generated'
.\uml.ps1 mutate 'business tree/file with spaces.lua' --busted-task fast --max-workers 3
```

External `crap` adds `--full-tree`: crapper publishes every class under the
specified roots in one `.metrics/crap.edn` snapshot. Roots default to the project
root, must exist inside it, and exclusions are project-relative paths or globs.
The tool rejects positional class paths, `--changed` and existing-coverage reuse
in a full-tree refresh. A failed refresh preserves the previous snapshot rather
than replacing other classes' metrics with a partial result. `--no-coverage`
explicitly requests complexity only; it does not establish fresh coverage.

Mutator currently accepts repeated `--source-root` but **has no `--exclude`
option**. The standard scope adapter supplies selected business files rather
than passing unsupported exclusions. Without a `standardTools` declaration the
scope is the project root; declare roots/exclusions or supply changed
business files explicitly. Crapper exclusions alone do not apply to mutation.
`mutate` stays
differential; only explicitly passing `--mutate-all` requests all sites. Passing `--max-workers 3`
to `./uml mutate src/calc/init.lua` caps parallel workers without changing
differential selection. Mutator writes the same
`.metrics/mutate/<namespace>.edn` snapshots used by the overlay, including
private Lua functions as `defn-/…`.

The standard contracts were inspected read-only in local crapper `1417c43`
and Mutator `844a0d7`. CRAP exit 0 means analysis completed; exit 2 means a
threshold finding after a valid snapshot was published; exit 1 denotes invalid
arguments or analysis/coverage/publication failure. Mutation exit 0 includes
uncovered sites recorded as coverage gaps; exit 3 means survivors with published
snapshots; exit 2 means baseline failure; usage and infrastructure errors are
failures. Findings retain snapshots and never request an automatic rerun or
`--mutate-all`. Snapshot joins remain by class namespace, then class id; missing
CRAP or mutation remains red.

Run `python -I scripts/test_standard_quality.py` for native public-entry checks.
The optional read-only sibling crapper check uses its installed dependencies to
verify actual roots, exclusions, complete-tree publication and preservation on
invalid partial refresh. These checks ran on Windows; macOS remains unverified.
They do not establish real Lua coverage/Busted or companion mailbox acceptance,
which remains part of consumer integration.

To run the isolated entry acceptance suite (Python 3 + PowerShell 7 + git):
`python -I scripts/test_entries.py`.
Set `UML_ENTRY_TEST_TMP` to choose a scratch root. This suite never launches the
real viewer.

`./uml ir` runs the LOVE IR generator on `examples/<dir>.policy.edn` (else the
first `examples/*.policy.edn`, else `policy.edn`). `./uml crap` and
`./uml mutate` run the project-local crapper and mutator. Run `./uml crap` on
the declared tree: `.metrics/crap.edn` is replaced every run. The companion uses
these three commands for every language.

`./uml` returns immediately and starts LOVE through the detached launcher.
Exceptions and viewer output append to **`uml-viewer-log.txt`** in the project
directory (gitignored).

A **Herdr** session unique to the examined project starts interactive Claude
Code in
that directory (`--dangerously-skip-permissions --append-system-prompt …`
plus a launch prompt). The name is
`uml-viewer-<project>-<hash>`, stored in `.uml-viewer/companion.edn`. On start
it writes a hierarchical policy from that project's namespaces and regenerates
the IR. Type there; Esc is the real TUI interrupt. Closing the diagram kills
**only that** companion session and its terminal window — other Claude sessions
stay
up. Verified abnormal Claude exits use the bounded recovery policy described
above; normal or unknown exits stop recovery. That
instance also runs `./uml crap`, `./uml mutate`, and `./uml ir` after later
changes. Project-wide rules live in `AGENTS.md`.

Prefer `./uml` in the examined project (from `get-uml-viewer`).

| Command | Who | What |
|---------|-----|------|
| `./uml` | anyone | Fresh LOVE window in the background. Starts the companion. Waits for `:display`. |
| `./uml --restart` | **associated agent only** | New LOVE process, same companion. Restores the last view. |

Do **not** pass `--restart` unless you are that companion recycling the
window after source changes. The wrapper requires the existing launcher/LOVE
records and recorded companion ownership, and refuses restart rather than
leaving a diagram with no agent or replacing a missing companion. The
companion recycles the window by writing `:quit-for-restart` to
`.uml-viewer/to-viewer.edn`, waiting for the LOVE viewer to exit, then running
`./uml --restart`. The new process restores the saved diagram path, discussion
context, depth, pan, zoom, and which proposal was showing
(`.uml-viewer/session.edn`). An explicit path after `--restart` opens that
diagram instead; omit the path to reuse the saved one. Only a legacy session
without a saved path falls back to the policy output. Policy selection matches
`./uml ir`: `examples/<project-directory>.policy.edn`, otherwise the first
`examples/*.policy.edn` sorted by filename, otherwise root `policy.edn`.
Relative `:out` paths resolve from the project root; absent `:out` uses the IR
default `examples/uml-viewer.edn`. Missing or malformed policy, invalid output,
and unreadable diagrams fail explicitly. A saved or explicit path that fails
to load does not trigger fallback. A launcher record from the removed JVM
runtime is refused. Do not SIGKILL.
Closing the window kills only that project's companion, not other Claude
sessions.

On a fresh start the canvas stays blank until the companion sends `:display`,
with **Waiting for agent to create diagram.** `R` reloads the current EDN
immediately and does not wait. A missing or unreadable file prints
`UML viewer: file not found: …` in the inspector instead of throwing.

`lua-fixture/` is a Lua project with `.metrics/` snapshots written by
crapper and mutator. The Lua specs scan it and check the overlay joins
those snapshots.

```powershell
scripts\test_lua.ps1                                   # LuaJIT + Lua 5.4 busted
& 'C:\Program Files\LOVE\lovec.exe' .\love ir .\examples\uml-viewer.policy.edn
.\uml.ps1 crap                                         # .metrics/crap.edn
.\uml.ps1 mutate love/uml/engine/layout.lua            # differential mutation
```

This repository's `uml-quality.json` declares
`standardTools.sourceRoots: ["love/uml"]`; the dispatcher runs the
project-local crapper and mutator. The full export pipeline is
`crapper` (CRAP + coverage) and `mutator` (differential mutation), both
installed by `get-uml-viewer` into `.uml-viewer/<tool>/.venv`. Commit
`.metrics/` so a clone has numbers without re-running those tools.

Rename or move of a function is a new form: overlay does not match old names.

The `fixtures/jvm-goldens/` files are the frozen parity oracle captured from
the removed JVM viewer. They are still read by the LOVE parity specs; only the
JVM regeneration tooling was deleted.

### Linux / WSL2

The integrated companion workflow now rejects Linux/WSL before creating
resources. The former tmux/WSLg and Windows Terminal bridge have been retired.
Run `./uml` and Herdr from native Windows instead (macOS is unsupported this
round); Linux/WSL has no
validated Herdr companion path in this release. Source scanning and standalone
`ir`/`crap`/`mutate` tools are separate from companion launch, and their own
platform requirements still apply. Existing legacy resources and queues are
retained; follow the migration instructions before starting fresh.

## Navigation

**Layer** and **component** mean the same thing: a namespace grouping
(the first segment after the prefix, or a named proposal group).

- First view: **namespace components** (layers). Dependencies between them
  collapse to one arrow. Each component lists nested namespaces.
- Double-click a component to open the next level. Esc or the ← label goes
  up a level, including after drilling a proposal group. Esc does not quit.
  The window close box or a `:quit-for-restart` mail message exits the app.
- Hover an arrow for a popup of every `from -> to` it bundles, in any
  declutter mode. Violating pairs are red.
- Right-click a class or component for **Refresh CRAP**, **Refresh
  Mutation**, **Refresh All Mutation**, or **Omit**. Each writes
  `to-agent.edn` and wakes the companion. CRAP runs on that class or the
  files under that component. Mutation is differential `./uml mutate` on
  those src files; All Mutation passes `--mutate-all`. Omit adds the id to
  the current proposal's `:omit`, or to policy `:omit` on the real diagram.
- Component titles paint **on top of** crossing arrows.
- The inspector lists the **real diagram** (the namespace tree) above
  **Proposals**. Click the real row to return to the tree; click a proposal
  to show it (marked as not in the code). Either click writes `:context`
  so the companion treats that diagram as the discussion. **New Proposal**
  adds a timestamp-named proposal. Right-click to rename or delete.
  **Declutter** cycles Declutter arrows / Remove arrows / Declutter
  elements / Declutter classes / Declutter none. **Remove arrows** hides
  the lines and puts a triangle on the top (incoming) and bottom
  (outgoing) of each box, **including nested classes** as well as their
  packages. A triangle is red if any bundled pair is violating. Hover it
  for the `from -> to` list.
- Double-click a leaf module for its **class card**.
- The class card names the **module** (`:ns`). Click it to open that source
  file at the top. Hover a member to highlight it; click it to open the same
  file at the declaration. See [Source extractors](#source-extractors).
- Methods on the card are `+` public and `-` private. Private functions are not
  drawn on the class box.
- Abstract classes show a white **α** in the upper-right; interfaces a white
  **I**. Names of rectangles that are not classes (components/layers,
  interfaces, enumerations, package banners) are italic. Foreign libraries
  listed in policy are ovals outside the components.
- The main window is resizable.
- Scroll to pan vertically; Shift-scroll (or left/right arrows) for
  horizontal. Pan can follow arrows that bow past the origin.
- **Ctrl+** (or **Ctrl+=**) zooms in 10%; **Ctrl-** zooms out 10%;
  **Ctrl+0** restores 100%. Zoom keeps the view center still.
- **Regen** in the inspector asks the companion to rewrite policy and IR
  (see [Companion mailbox](#companion-mailbox)).
- `R` reloads the current EDN. The watcher also reloads on save, and when
  `.metrics/` snapshots change. An open class card updates with the new
  numbers.
- `Esc` on the class card closes it (it does not quit the viewer). Closing
  the main window exits the app.

## Policy

This project's diagram is **generated**. Do not edit `examples/uml-viewer.edn`.
Edit `examples/uml-viewer.policy.edn`, then run `./uml ir` (or press Regen).

The **scanner** reads source and emits facts. One class is emitted per module.
A project-local import/require is a dependency. External requires/imports
become **foreign** classes. The overlay fills members from `.metrics/`.
TypeScript, Rust, Python, and Lua each have a scanner (see
[Language graphs](#language-graphs)). This repository's policy uses
`:lang :lua` with `:src "love"` and `:prefix "love"`, so class
namespaces are `love.uml.*` and match crapper's project-relative snapshot
namespaces. The overlay joins each snapshot by `:ns`, then by the class id.

### Do not invent layers (components)

The tree **is** the namespaces. After `:prefix`, every `.` is a nesting
level. `love.uml.engine.layout` is a child of `uml.engine`.
`love.uml.lua_language.source_lua` is a child of `uml.lua_language`.
The policy does **not** assign nses to invented packages. If you want Domain /
Engine / Adapters boxes **in the source tree**, those segments must exist as
namespaces. To **view** a grouping that is not in the code, use `:proposals`
(see [Proposed components](#proposed-components)) — do not rewrite namespaces.

To write a policy for a project:

1. Set `:prefix` to the namespace prefix (`love` for `love.uml.graph`). A
   single language also sets `:src` (default `src`) and `:lang`. More than
   one language sets `:sources` instead of `:src`.
2. Set `:hierarchical true` (or omit `:packages` and `:diagrams`).
3. List top-level **segments** in `:order` — the first dotted part after
   the prefix, in the order you want the boxes. Do not invent names.
4. List real libraries in `:foreign` if they should appear as ovals.
5. Optionally override a require with `:edge-kinds {[:from :to] :association}`
   using the **leaf** ids (`lua_language.source_lua`, not
   `lua_language`).
6. Set `:levels` so the generator can mark dependency-rule violations
   (see [Dependency rule](#dependency-rule)).
7. Optionally set `:proposals` to name design components that are not namespaces
   (see [Proposed components](#proposed-components)).
8. Run `./uml ir` (or Regen).

If `foo.bar` and `foo.bar.baz` both exist, the `bar` box lists `bar` (the
module) and `baz` (the child). Module titles are the last namespace
segment (`layout`, not `engine.layout`). A hyphenated segment capitalizes
each part (`source-lua` → `SourceLua`). A segment that already
contains a capital keeps that spelling and capitalizes the first letter
(`tauriFs` → `TauriFs`). Double-click the component to open that level;
double-click the `bar` module line for its class card.

Wrong (invented partitions):

```edn
:packages [{:id :domain :nses [ir geom source]}
           {:id :engine :nses [layout route]}]
```

Right (the ns tree):

```edn
{:title "UML viewer"
 :src "love"
 :prefix "love"
 :lang :lua
 :out "examples/uml-viewer.edn"
 :hierarchical true
 :proposals [{:id :p-1791033119418, :name "test", :layers []}]
 :foreign [lfs ffi cimgui]
 :order [main conf uml.main uml.adapters uml.application uml.engine
         uml.source uml.graph uml.lua_language uml.typescript_language
         uml.rust_language uml.python_language uml.edn uml.compat uml.domain]
 :levels [[uml.domain uml.source uml.graph
           uml.lua_language uml.typescript_language
           uml.rust_language uml.python_language]
          [uml.engine]
          [uml.application]
          [uml.adapters]
          [uml.main main conf]]}
```

A project with more than one language lists each tree under `:sources`.
The policy `:prefix` is stripped from ids. A source `:prefix` is that
tree's namespace root. `src/model.ts` becomes `bookwriter.model`
(`:model`). `src-tauri/src/lib.rs` becomes `bookwriter.rust` (`:rust`).
`main.rs` becomes `bookwriter.rust.main` (`:rust.main`).

```edn
{:title "Bookwriter"
 :prefix "bookwriter"
 :sources [{:lang :typescript :root "src"}
           {:lang :rust :root "src-tauri/src" :prefix "bookwriter.rust"}]
 :foreign [at.tauri-apps markdown-it highlight.js node tauri serde std]
 :hierarchical true}
```

| Key | Role |
|-----|------|
| `:prefix` | Strip this from each ns; remaining dots are the tree |
| `:hierarchical` | Namespace tree (default when `:packages` is omitted) |
| `:order` | Order of **existing** top-level ns segments, not new component names |
| `:levels` | Groups of those segments, **inner (higher-level) first**. Same group = same rank |
| `:proposals` | Named groupings of real segments; **not** instantiated in source. Inspector **Real diagram** row returns to the ns tree |
| `:omit` | On a proposal or the policy: nses (and their children) left off the diagram |
| `:edge-kinds` | Override parser kind for `[from to]` (usually `:association`) |
| `:omit-edges` | Drop `[from to]` |
| `:lang` | Scanner when `:sources` is absent (`:lua`, `:typescript`, `:rust`, or `:python`) |
| `:src` | Source root when `:sources` is absent |
| `:sources` | One scan per `{:lang :root :prefix?}`. The entry `:prefix` is that tree's namespace root |
| `:foreign` | External libs as ovals. A listed prefix collapses `at.tauri-apps.api` to `at.tauri-apps`. Unlisted externals are dropped |

**Viewer Claude loop** (passed with `--append-system-prompt` to the
companion session only)

On launch: from the examined directory, write or update the hierarchical
policy and regenerate the IR, then wait.

After **every** later source or policy change, refresh metrics and
regenerate the IR.

For this repository that is `./uml crap`, then `./uml mutate` on the changed
source files, then `./uml ir`. Uncovered mutants that remain are coverage
gaps. Keep the snapshot. Do not re-run the file or force a full mutation
because mutate exited non-zero.

For another language, run that language's CRAP and mutation tools when
they exist. The overlay joins each snapshot by `:ns`, then by the class
id. Missing CRAP or mutation data is red. Then regenerate the IR.

- Add/rename/delete a namespace: the tree updates on `./uml ir`. Put a new
  top-level **segment** in `:order` if you care about box order.
- Nested nses appear as contents of the parent component.
- “This require is really an association”: one `:edge-kinds` entry.
- Show a library like cimgui as an oval: add it to `:foreign`.
- Do not add `:packages` to fake Clean Architecture components. Use
  `:proposals` to view a grouping that is not in the code.
- Preserve `:proposals` when rewriting policy. Do not invent them on launch.
  If instructed, add a named proposal (default name is a timestamp).

Hand-written sample IRs (e.g. `examples/library.edn`) are still valid; they
are not generated.

### Dependency rule

A `:dependency` edge is **violating** when it runs from a **higher-level**
(inner) component to a **lower-level** (outer) one. That is the Clean
Architecture dependency rule: source-code dependencies point inward.

Evaluation is deterministic given `:levels`:

1. Take the first dotted segment of each end (`engine.layout` → `engine`).
2. Look up that segment in `:levels`. Rank is the group's index; **smaller
   is inner / higher-level**.
3. If both ends have a rank and `from-rank < to-rank`, the edge is
   `:violating true`. Same rank is allowed. `:implements` and
   `:association` are never violating. Foreign / unranked ends are not
   compared.
4. Collapsed component arrows keep the flag if any bundled leaf dependency
   was violating. Remapping a pair to `:association` clears it.

`:order` is visual box order, not rank. Nesting is not layering: you cannot
infer inner vs outer from the namespace tree alone, so `:levels` must group
segments that sit at the same architectural level (e.g. `domain`, `source`,
and `graph`). Omit `:levels` and nothing is marked. If `:levels` is omitted
and `:proposals` is set, rank follows the first proposal's component order.

### Proposed components

`:proposals` is a list of named groupings of **existing** top-level segments.
Those names are not namespaces. Each item is `{:id :name :layers [...] :omit [...]}`
(`:layers` here are named components; `:omit` keeps listed nses off
**Unassigned**). The as-is diagram stays the ns tree.
The inspector lists the real diagram (the namespace tree) just above
**Proposals**; click it to return to the tree. Click a proposal to show it
(canvas marked **PROPOSAL — not instantiated in code**). Either click
tells the companion that diagram is the context of discussion. **New
Proposal** adds an empty proposal named with a timestamp. Right-click a
name to rename or delete it. Double-click a ns box to drill; **←** at the
top returns to the proposal.

The **Declutter** button cycles **Declutter arrows** (one arrow per
component pair per direction) → **Remove arrows** (triangles on each box
and on nested classes instead of lines; hover lists deps; red if any pair
is violating) →
**Declutter elements** (also hide nested names, members, and ports) →
**Declutter classes** (also hide classes inside components) →
**Declutter none**.

When a proposal is shown, rank follows **that** proposal's component order
and violating arrows are re-evaluated. The real diagram uses `:levels`.
A component inherits the **max** level of its elements.
Class boxes show the current view's rank (innermost **0**)
at the upper left; the class card repeats **Level n**. Level 0 is drawn at
the **bottom**. Good arrows (outer → inner) point down; violating arrows
(inner → outer) point up and stay red. When arrows are collapsed,
selecting a class highlights the component arrows it belongs to.
Collapsed components keep their color
and C/M dots; double-click still opens a component.

```edn
:proposals [{:id :ccp
             :name "2026-09-18 10:30:00"
             :layers [{:id :playfield :label "Playfield"
                       :nses [entities world missiles cities batteries flyers]}
                      {:id :hosts :label "Hosts" :nses [jvm browser]}
                      {:id :jvm :label "JVM"
                       :nses [jvm.cli jvm.main
                              {:id :quil-swing :label "Quil/Swing"
                               :nses [jvm.sketch jvm.window]}]}]
             :omit [cli]}]
```

The companion must not invent `:proposals` on launch and must keep them when
updating `:order`. If instructed, add a named proposal and regenerate the IR.

The viewer draws a violating arrow **red**, and **bold red** when a selected
element highlights it. Hand-written IR may set `:violating true` directly.

## Companion mailbox

The viewer and the companion Claude talk through `.uml-viewer/` in the examined
project (gitignored). The file is the mail; Herdr delivers the wake-up.

| File | Direction |
|------|-----------|
| `.uml-viewer/to-viewer.edn` | Claude → viewer |
| `.uml-viewer/to-agent.edn` | viewer → Claude |

Each mailbox file is a small queue `{:next-id n :queue [cmd …]}` (tmp-then-rename).
Commands have a rising `:id`. Append; do not overwrite. Handling a command
**pops** it from `:queue` and rewrites the file (oldest first). The viewer
does this itself on `to-viewer.edn`. The companion must pop each
`to-agent.edn` command as it handles it.

`.uml-viewer/session.edn` is the last view (diagram path, discussion context,
depth, pan, zoom, proposal, and mail cursor) written on `:quit-for-restart`
and restored by `--restart`. `.uml-viewer/viewer-process.json` records the
launcher identity, and `.uml-viewer/viewer-process-identity.json` records the
exact LOVE PID/start; the public wrapper verifies and waits for those
identities before starting a replacement. `.uml-viewer/viewer-jvm.json` is no
longer recognized. `.uml-viewer/companion.edn`
records the Herdr session, socket, workspace, pane, server/shell process
identities and recovery-controller reference. Each operation verifies this
project's resources before acting; other projects and the shared Herdr service
remain untouched. Terminal attachment failure retains the companion and prints
an attach command to the project log.

| `:op` | Meaning |
|-------|---------|
| `:display` | Viewer loads `:path` (relative to the project root). Canonical paths identifying the current diagram reload IR/metrics and retain its valid view; a different diagram starts at its own root. |
| `:regen` | Claude rewrites hierarchical policy, regenerates IR, then `:display` |
| `:quit-for-restart` | Viewer saves the diagram path, discussion context, view, and mail cursor, then exits the LOVE process without killing Claude. The associated agent waits for exit and runs `./uml --restart`. |
| `:refresh-crap` | Public `./uml crap` with declared scope (standard tools: whole business tree; operation override: targets or whole tree); valid result → successful IR → display |
| `:refresh-mutate` | Public `./uml mutate` on target files, differential; valid results → successful IR → display |
| `:refresh-mutate-all` | Public `./uml mutate --mutate-all` on target files only; valid results → successful IR → display |
| `:omit` | Add `:target` `:id` to proposal or policy `:omit`, then IR |
| `:context` | Inspector selection is the discussion context: `{:context :real}` or `{:context :proposal :proposal-id id :name "…"}` |

Quality refreshes share CLI dispatch and its configured scope/exclusions. Install
an absent public entry with `get-uml-viewer --install-only`. Process mail oldest
first, popping and rewriting the queue; keep real/proposal discussion context
until later context mail, and preserve proposal definitions and omit semantics.
Invoke mutation once per selected source file, never a file list. Scoped
mutation passes selected files without adding source-root flags.

Before each serialized quality invocation remove the previous
`.uml-viewer/quality-result.json`, await completion and capture the actual exit.
The newly published record must have version 1, the requested operation, this
canonical project root, the actual exit code and `validResult: true`. Require
all component results to pass before IR. Missing, stale, malformed or mismatched
records and invalid results stop that request's IR/display and report the cause.
IR must itself exit 0 before display. Continue later FIFO requests after a
failure. Recovery follows the same gate and never displays a failed refresh.
Survivors and uncovered mutants retain their snapshots; they do not trigger
retries or implicit full mutation.

The optional version-1 `uml-quality.json` declares `standardTools` or
complete `operations` overrides. The Clojure alias route and
`aliasResultExitCodes` were removed with the JVM implementation. Standard
CRAP accepts exit 0 (analysis completed) and 2 (threshold finding after a
valid snapshot); standard mutation accepts 0 (including uncovered coverage
gaps) and 3 (survivors with published snapshots). Exit 3 is never universally
accepted: it requires the tool contract or an explicit project operation
declaration. Direct tool commands publish no wrapper result record.
Noninteractive wrapper and application tests establish these boundaries;
actual Windows Herdr right-click/FIFO/display acceptance remains pending manual
validation. macOS is unsupported this round.

Same-diagram display keeps focus, pan, zoom, proposal/open layer, declutter,
and valid selection/detail cards. A missing focus falls back to the nearest
remaining namespace ancestor; a missing layer returns to proposal root; a
missing proposal returns to the namespace tree. Missing selection/card targets
are cleared. Display preserves discussion context until another `:context`
arrives. Automated application specs cover this refresh contract; combined
Windows companion right-click acceptance is pending, and native macOS acceptance
has not been run.

Clicking **Real diagram** or a proposal (including **New Proposal**) writes
`:context` and wakes Claude. Stay on that architecture until the next
`:context`.

Right-click ops include `:target {:id :ns :kind :class|:component :proposal-id?}`.
`:kind` is `:component` for a layer box (and its nested nses) and `:class`
for a module.

**Regen** in the inspector queues `:regen` before asking Herdr to wake the
verified companion. The wake-up tells Claude to consume the FIFO mailbox; if
Claude is busy, it finishes first. A failed wake keeps the queued request and
the inspector reports that the session is not attached.

## Language graphs

Generating the IR asks `love/uml/graph.lua` to scan a source tree. `:lang`
selects the scanner; `:sources` scans each root and merges them. A scanner is
registered in `love/uml/main/ir_generator.lua` and must satisfy
`LanguageGraph`:

| method | role |
|--------|------|
| `scan` | From a root and `{:prefix :ns-prefix?}`, return `{:classes :edges}` |

`:commands` and `:invokes` are optional. `merge_scans` turns a matching
command name into a `:dependency` and does not copy those keys onto a
class.

Classes are `{:id :name :ns :stereotype :lang :file}`. Edges are
`{:from :to :kind}` (`:dependency` or `:implements`). The policy layer is
language-neutral. Do not special-case a language in `policy`. Main
requires each scanner so it registers; `ir-generator` looks the scanner
up.

**TypeScript** (`love/uml/typescript_language/graph_typescript.lua`) emits
one class per module file (`.ts`, `.tsx`, `.js`, `.jsx`, `.mjs`, `.cjs`).
It skips `*.test.*`, `*.spec.*`, `*.d.ts`, and paths under `node_modules`,
`dist`, and `target`. A relative import is a dependency, and `import type`
counts. An asset import (CSS, images, JSON, and the like) is ignored. A
bare specifier is foreign: `@tauri-apps/api/core` becomes
`:at.tauri-apps.api.core`. A listed `:foreign` prefix such as
`at.tauri-apps` collapses that id. `export function f(): T` and
`export const x: T` are `:implements` when `T` is an imported project
name. `Promise<Book>` and `TreeNode[]` are not. A module that exports an
interface and no values is `:stereotype :interface`. Exported functions,
constants, classes, and enums are `:ops`. `invoke("name")` is recorded
for the cross-language link.

**Rust** (`love/uml/rust_language/graph_rust.lua`) walks the modules
reachable from `lib.rs` and `main.rs` through `mod`. The `[lib] name` in
`Cargo.toml` is the crate name, so `bookwriter_lib::run()` in `main.rs`
depends on the lib class. Every Rust class is named by its source
file. The directory box keeps the namespace segment. `pub fn` functions
are `:ops`. `pub(crate)` functions are not,
unless they are `#[tauri::command]` functions, which are `:ops` even when
private. `impl Trait for Type` is `:implements` when the trait resolves
to a project module. The first segment of a `use`, other than `crate`,
`self`, `super`, or the lib crate, is a foreign crate.

**Python** (`love/uml/python_language/graph_python.lua`) emits one class per
`.py` module. `__init__.py` is the package. Point `:src` at the package
directory or at its parent. A path that already begins with the
prefix stays as written, so `src/tools/parse.py` and
`src/tools/__init__.py` become `tools.parse` (`:parse`) and `tools`
(`:tools`). It skips `test_*.py`, `*_test.py`, `*_tests.py`,
`conftest.py`, and paths under `__pycache__`, `tests`, virtualenvs,
`dist`, and `build`. `import` and
`from ... import` are dependencies, including an import nested in a
function. A relative import (`from . import`, `from ..mod import`) stays
inside the package. A name that is not a project module is foreign
(`os.path` stays `os.path`, and a listed `:foreign` prefix such as `os`
collapses it). `class Dog(Animal)` is `:inheritance` when `Animal` is a
project module. A base of `Protocol` or `ABC` is `:implements`.
`def load() -> Animal` is `:implements` when `Animal` is a bare imported
project name. `list[Animal]` is not. Public module-level functions and
classes are `:ops`. A module whose public classes are only `Protocol` or
`ABC` bases, with no public functions, is `:stereotype :interface`.

**Lua** (`love/uml/lua_language/graph_lua.lua`) emits one class per `.lua`
module. The module name is the `require` path: strip the longest of
`src/lua/`, `src/`, `lua/`, drop `.lua`, collapse a trailing `/init`,
then `/` becomes `.`, so `src/calc/init.lua` and `src/calc/util.lua`
become `calc` (`:calc`) and `calc.util` (`:calc.util`). It skips
`*_spec.lua` and paths under `spec`, `lua_modules`, `dist`, `build`,
`target`, and `.metrics`. `require "a.b"`, `require("a.b")`, and
`require 'a.b'` are dependencies; a name that is not a project module is
foreign (`lfs` stays `:lfs`). Comments, strings, and long brackets are
masked, so a `require` inside them is ignored. Top-level functions that
are not `local` are `:ops`, named as written in the source (`M.clamp`,
`Account:deposit`). A function declared inside another is not an op.
A top-level `setmetatable(t, {__index = Base})` or
`setmetatable(t, Base)`, where `local Base = require("app.base")` names a
project module, is `:inheritance` on that module. This is a pattern match,
not an evaluation: a `setmetatable` inside a function makes an instance and
draws nothing, and a base that is not a local bound by `require` is not
followed. There are no `:implements` edges and no interface stereotype.

`merge-scans` links a TypeScript `invoke("read_text")` to the Rust class
that owns `#[tauri::command] fn read_text`, as a `:dependency`. Two
project classes with the same id are an error. When a dependency and an
`:implements` edge join the same pair, the IR keeps `:implements`.

CRAP and mutation for TypeScript, Rust, Python, and Lua come from separate
tools. The overlay joins a snapshot to the class whose `:ns` equals that
namespace. Otherwise the class id owns that name (`bookwriter.model`
owns `model`), a dotted child rolls up (`pdf` owns `pdf.Layout`), and
the policy prefix belongs to the single undotted Rust class. `::` is
read as `.`. A source file drawn inside its directory keeps its own
CRAP and mutation scores. The directory box pools the functions under
it, one weight per function. A file with no snapshot stays red, and in
that pool it counts as one red function per op, or one function when it
lists none. A class with no CRAP or mutation data is red.

## IR

A hierarchical policy writes one EDN document of all classes and edges
(`:hierarchical true`). The viewer builds each screen from the namespace tree
at the current drill level. A hand-written IR with `:packages` (or
`:diagrams`) is still a static diagram, e.g. `examples/library.edn`.

Metrics on the class card do not have to be authored. If `.metrics/` is
present, the overlay fills CC, coverage, CRAP, killed/survived/uncovered,
and any functions found in the snapshots (including privates). Authored
`:crap` / `:coverage` / `:ops` are the fallback when no snapshot exists.

The overlay joins a snapshot to the class whose `:ns` equals that
namespace. The generator writes `:ns` from the scanned ns. Hand-written
IR must set `:ns` the same way. When no class has that `:ns`, the class
id, a dotted child of that id, and the policy prefix on the single Rust
crate root are the fallbacks.

```edn
{:title "Lending library"
 :direction :tb
 :packages
 [{:id :domain
   :label "Domain"
   :classes
   [{:id :book
     :name "Book"
     :stereotype :class          ;; optional: :interface :enumeration :abstract
     :fields [{:name "isbn" :type "String"}]
     :ops [{:name "find" :args ["isbn"] :returns "Book"}]}]}
  {:id :app
   :label "Application"
   :classes
   [{:id :repo
     :name "CatalogRepo"
     :stereotype :interface
     :ops [{:name "get" :args ["isbn"] :returns "Book"}]}]}]
 :edges
 [{:from :sql-repo :to :repo :kind :implements}
  {:from :loan :to :book :kind :association :label "borrows"}]}
```

Optional authored metrics, used when snapshots are missing:

- `:crap` — a number (`μ`) or `{:mu :max :sigma}`
- `:coverage` — ratio 0–1 on a class or op
- `:cc`, `:killed`, `:survived`, `:uncovered`, `:private` on ops
- `:hide-members true` — compact box

Package and class **color** maps CRAP (`μ + σ`) and mutation score each onto
1–10 using `love/uml/domain/config.lua` cutoffs, averages them, and paints a
0–10 red–green fill. Missing CRAP or mutation data counts as red (grade 1),
not unknown. A parent pools CRAP by function count: μ and σ of the
functions underneath, then the same grade. It pools mutation by summing
killed and survived. A child with no data tints that pool once per
function. It does not replace the parent. A **C** and **M** dot in
the upper-right show the two scores. The boxes no longer print μ / max / σ.

On the class card, a `Crap μ … max … σ …` line sits above the table (max is
the worst function in the namespace, not a sum). Column groups are labeled
`--crap--` (Crap, CC, Cov) and `--mutation--` (killed, survived, uncovered).
The class row shows average CRAP with a `μ` suffix and omits CC. Killed is
white. Survived and uncovered are green at 0 and red when nonzero. A row
with no mutation operators (`:sites` 0) shows `---no mutation sites---`.
A form with operators still shows killed/survived/uncovered, including zeros.

Edge `:kind` values:

| kind | line | head |
|------|------|------|
| `:inheritance` | solid grey | empty triangle |
| `:implements` | solid grey | empty triangle |
| `:association` | solid grey | open arrow |
| `:dependency` | solid grey | open arrow |
| `:aggregation` | solid grey | empty diamond |
| `:composition` | solid grey | filled diamond |

Layout follows Mermaid's three stages:

1. **Size** each class from its text (padding 12).
2. **Place** packages in document order; classes Sugiyama-ranked inside a
   package; ~40px spacing.
3. **Route** like Mermaid/ELK: ports on facing sides, orthogonal tracks in
   the rank gap, short same-rank connections through the stack gap (local U
   only when a sibling sits in the way), then stroke with D3 `curveBasis`
   cubics. Ordinary arrows are solid grey; violating dependencies are red
   (bold red when selected). Component titles draw after the arrows so
   names stay visible. Paths that pass the target and reverse are rejected.

## Source extractors

Clicking a member asks `love/uml/source.lua` for the **whole file** and a
**start line**. The IR (and the class card) only supply an **identity
map**; a language-specific extractor turns that into
`{:title :file :body :line}`. The source window opens on that file and
scrolls to the member (highlighted). Clicking the module name opens the same
file at the top (`:line` omitted).

```lua
Source.member_source({lang='lua', file='love/uml/engine/layout.lua', name='layout'}, files)
```

`:lang` selects the extractor; there is no Clojure extractor after the cutover.
A scanner is registered in `love/uml/main/source.lua` and must satisfy
`LanguageSource`:

| method | role |
|--------|------|
| `locate` | path to the file that should contain the member |
| `extract` | slice that member out of the file text |
| `title` | window title |

**TypeScript** (`love/uml/typescript_language/source_typescript.lua`),
**Rust** (`love/uml/rust_language/source_rust.lua`), **Python**
(`love/uml/python_language/source_python.lua`), and **Lua**
(`love/uml/lua_language/source_lua.lua`) open `:file` and find the
exported declaration, the `fn`, the `def`, or the `function`. The class card passes
`:lang` and `:file` from the class. The protocol is the seam; do
not special-case languages in the class card.

cimgui stays in `adapters.class_boxes`, `adapters.edges`, and
`adapters.inspector`. The rest of the engine does not depend on the ImGui
binding.
