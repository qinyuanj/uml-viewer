-- Pure companion policy: executable selection, read-only ownership preflight
-- and the verbatim prompts handed to Claude. No filesystem or process access.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local M={}

function M.fail(message,data)
  error(setmetatable({message=message,data=data or {}},{__tostring=function(e) return e.message end}),0)
end

M.legacy_guidance="tmux/psmux companions are retired. Ownership and resources were retained; no takeover or cleanup was attempted. Finish the old viewer/session using the old release, verify that this project's legacy resources have exited, then archive .uml-viewer/companion.edn and start ./uml fresh with Herdr. Do not use --restart to migrate; retain session.edn and mailbox queues."
M.herdr_guidance="Herdr 0.9.3 or newer (protocol 22) is required. Install a release from https://github.com/herdrdev/herdr/releases (https://herdr.dev). No automatic download, upgrade or fallback. WSL/Linux is the delivered platform (spec #106); macOS Apple Silicon is implemented but awaits real-machine acceptance; Windows native is unsupported (ADR 0007). Terminal attach is unverified (#49)."
M.recovery_prompt="Recovery: do not scan sources or rewrite policy. Read the current .uml-viewer/to-agent.edn queue; pop and handle commands oldest first, preserving the latest context and existing proposals. Apply the standing quality-result gate to each refresh; only valid results followed by successful IR permit :display. A failed refresh stops that request's IR/display, reports its cause, and leaves later FIFO mail runnable. If there is no pending refresh, restore :display from .uml-viewer/view.edn; never issue an unconditional recovery display after a failed refresh. Then wait for directives."
M.standing_rules=[=[You are the UML-viewer companion. The current working directory is the
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
./uml ir runs the LOVE IR generator on this project's policy. Use the public
./uml crap and ./uml mutate quality dispatch for mailbox refreshes: it
shares CLI routing, optional uml-quality.json overrides and standard tool
scope/exclusions. If the entry is absent, install it
with get-uml-viewer --install-only before refreshing; direct tool commands
do not publish a result record.
2. Run ./uml crap with declared scope: standard tools cover the whole
   business tree; an operation override may declare targets scope.
   Preserve unrelated class metrics under the selected tool contract.
3. Run ./uml mutate on each changed source file (differential).
   Invoke mutation once per source file, never a file list.
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
and ./uml --restart (new LOVE viewer, keep this session, restore last view).
The viewer runs on LOVE 11.5 with cimgui-love; there is no JVM or Clojure
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
]=]
M.launch_prompt="On launch: from this working directory, update the hierarchical policy to match the project's namespaces (no invented layers/components), regenerate the IR, then wait for directives."

local function map_kind(value) return type(value)=='table' and E.kind(value)=='map' end
local function get(record,key)
  if not map_kind(record) then return nil end
  return G.get(record,key)
end
local function name(value)
  local kind=E.kind(value)
  if kind=='keyword' or kind=='symbol' then return value.name end
  return value
end
local function present(record,key)
  local value=get(record,key)
  return value~=nil and value~=E.nil_value
end

function M.herdr_version_supported(version)
  if type(version)~='string' then return false end
  local major,minor,patch=version:match('^(%d+)%.(%d+)%.(%d+)$')
  if not major then return false end
  local parts={tonumber(major),tonumber(minor),tonumber(patch)}
  local floor={0,9,3}
  for i=1,3 do
    if parts[i]~=floor[i] then return parts[i]>floor[i] end
  end
  return true
end

function M.recorded_backend(record)
  if not map_kind(record) or #record==0 then return nil end
  local backend=name(get(record,'backend'))
  if backend==nil or backend=='tmux' or backend=='psmux' then
    M.fail(M.legacy_guidance,{status='failure',reason='legacy-ownership'})
  end
  return backend
end

function M.selection_args(args)
  local forwarded={} ; local selected
  local i=1
  while i<=#args do
    local arg=args[i]
    if arg=='--backend' then
      local value=args[i+1]
      if value=='tmux' or value=='psmux' then
        M.fail(M.legacy_guidance,{status='failure',reason='retired-backend'})
      end
      if selected~=nil or value~='herdr' then
        M.fail('Unknown or duplicate companion backend; use --backend herdr',{status='failure'})
      end
      selected='herdr'; i=i+2
    else
      forwarded[#forwarded+1]=arg; i=i+1
    end
  end
  return {args=forwarded,backend=selected}
end

function M.herdr_candidates(os,path,probe)
  local result,seen={},{}
  for part in (path or ''):gmatch('[^:]+') do
    part=part:gsub('"','')
    if part~='' then
      local candidate=part:gsub('/+$','')..'/herdr'
      local observed=probe(candidate)
      if observed and observed.executable then
        local canonical=observed.canonical
        if canonical and not seen[canonical] then seen[canonical]=true; result[#result+1]=canonical end
      end
    end
  end
  return result
end

local schema_pattern='^%s*{%s*"$schema"%s*:%s*"[^"]*"%s*,%s*"protocol"%s*:%s*22%s*,'

function M.probe_herdr(run,exe)
  local version=run{exe,'--version'}
  local release
  if version.exit==0 then release=version.out:match('^herdr (%S+)$') end
  if not M.herdr_version_supported(release) then
    return {path=exe,reason='not a herdr X.Y.Z >= 0.9.3',result=version}
  end
  local schema=run{exe,'api','schema','--json'}
  if schema.exit==0 and schema.out:find(schema_pattern) then
    return {path=exe,release=release}
  end
  return {path=exe,release=release,reason='bundled schema is not protocol 22',result=schema}
end

local function rank(release)
  local out={}
  for part in release:gmatch('%d+') do out[#out+1]=tonumber(part) end
  return out
end
local function greater(a,b)
  for i=1,3 do
    if a[i]~=b[i] then return a[i]>b[i] end
  end
  return false
end

function M.select_herdr(candidates,run)
  local probes={}
  for _,exe in ipairs(candidates) do probes[#probes+1]=M.probe_herdr(run,exe) end
  local best
  for _,candidate in ipairs(probes) do
    if not candidate.reason and (not best or greater(rank(candidate.release),rank(best.release))) then
      best=candidate
    end
  end
  if best then return {path=best.path,release=best.release} end
  local rejected={}
  for _,candidate in ipairs(probes) do
    rejected[#rejected+1]=candidate.path..' ('..(candidate.release or 'unknown')..': '..candidate.reason..')'
  end
  local detail=#rejected>0 and table.concat(rejected,'; ') or 'none'
  M.fail(M.herdr_guidance..' Candidates on PATH: '..detail,{status='failure',candidates=probes})
end

-- Read-only validation before any Herdr command or resource creation.
function M.preflight_checks(record,opts)
  opts=opts or {}
  if record~=nil and not map_kind(record) then
    M.fail('Present companion ownership is corrupt or unreadable; file retained',{status='failure'})
  end
  local recorded=M.recorded_backend(record)
  if recorded and recorded~='herdr' then
    M.fail('Unknown recorded companion backend; ownership retained',{status='failure'})
  end
  if map_kind(record) and #record>0 then
    local complete=true
    for _,key in ipairs{'socket','workspace','terminal-id','shell-pid','owner-start','server-pid','server-start'} do
      if not present(record,key) then complete=false end
    end
    if not complete or not present(record,'session') or get(record,'session')==''
        or not present(record,'pane') or get(record,'pane')==''
        or not present(record,'cwd')
        or name(get(record,'owner-role'))~='companion'
        or get(record,'provisional')==true then
      M.fail('Companion ownership is incomplete, provisional or belongs to another project; file retained',{status='failure'})
    end
    local canonical=opts.canonical or function(path) return path end
    if canonical(opts.root)~=canonical(get(record,'cwd')) then
      M.fail('Companion ownership is incomplete, provisional or belongs to another project; file retained',{status='failure'})
    end
  end
  if opts.requested and opts.requested~='herdr' then
    M.fail(M.legacy_guidance,{status='failure',requested=opts.requested})
  end
  if opts.restart and not recorded then
    M.fail('Companion ownership is missing; cannot restart',{status='failure'})
  end
  -- Herdr is the only backend, and only the POSIX platforms are delivered:
  -- WSL/Linux and macOS Apple Silicon. Windows native is retired (ADR 0007), so
  -- it is refused with that reason instead of the generic one.
  local platform=tostring(opts.os or '')
  if platform:sub(1,7)=='Windows' then
    M.fail('Windows native is not a supported platform (ADR 0007). Use WSL2 + WSLg'
      ..' or macOS Apple Silicon; ownership and queues are retained.',{status='failure'})
  end
  if platform~='Linux' and platform:sub(1,3)~='Mac' then
    M.fail('Unsupported companion platform: '..platform..'. Run the viewer and Herdr'
      ..' natively; ownership and queues are retained.',{status='failure'})
  end
  return recorded or 'herdr'
end

return M
