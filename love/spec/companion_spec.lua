local E=require('uml.edn')
local G=require('uml.domain.geom')
local C=require('uml.domain.companion')
local function map(t) return G.map(t) end
local function kw(name) return E.keyword(name) end
local function error_of(fn)
  local ok,err=pcall(fn)
  assert.is_false(ok,'expected an error')
  return err
end
local function installed(releases,calls)
  return function(argv)
    calls[#calls+1]=argv
    local entry=releases[argv[1]]
    if not entry then return {exit=1,out='',err='not found'} end
    if argv[2]=='--version' then return {exit=0,out='herdr '..entry[1]} end
    return {exit=0,out='{"$schema":"fixture","protocol":'..entry[2]..',"schema_version":1}'}
  end
end

describe('companion prompt text',function()
  it('ports the standing rules and launch prompt verbatim including the quality-result gate',function()
    local expected_standing=[[You are the UML-viewer companion. The current working directory is the
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
]]
    assert.are.equal(expected_standing,C.standing_rules)
    assert.are.equal('On launch: from this working directory, update the hierarchical policy to match the project\'s namespaces (no invented layers/components), regenerate the IR, then wait for directives.',C.launch_prompt)
  end)
  it('ports the recovery prompt and installation guidance verbatim',function()
    assert.are.equal('Recovery: do not scan sources or rewrite policy. Read the current .uml-viewer/to-agent.edn queue; pop and handle commands oldest first, preserving the latest context and existing proposals. Apply the standing quality-result gate to each refresh; only valid results followed by successful IR permit :display. A failed refresh stops that request\'s IR/display, reports its cause, and leaves later FIFO mail runnable. If there is no pending refresh, restore :display from .uml-viewer/view.edn; never issue an unconditional recovery display after a failed refresh. Then wait for directives.',C.recovery_prompt)
    assert.are.equal('tmux/psmux companions are retired. Ownership and resources were retained; no takeover or cleanup was attempted. Finish the old viewer/session using the old release, verify that this project\'s legacy resources have exited, then archive .uml-viewer/companion.edn and start ./uml fresh with Herdr. Do not use --restart to migrate; retain session.edn and mailbox queues.',C.legacy_guidance)
    assert.are.equal('Herdr 0.9.3 or newer (protocol 22) is required. Install a release from https://github.com/herdrdev/herdr/releases (https://herdr.dev). No automatic download, upgrade or fallback. Windows native acceptance is partial; macOS end-to-end acceptance passed with Herdr 0.9.4 (spec #32), terminal attach unverified (#49).',C.herdr_guidance)
  end)
end)

describe('Herdr version floor',function()
  it('accepts only dotted numeric X.Y.Z at or above 0.9.3',function()
    for _,version in ipairs{'0.9.3','0.9.4','0.10.0','1.0.0','10.0.0'} do
      assert.is_true(C.herdr_version_supported(version),version)
    end
    for _,version in ipairs{'0.9.2','0.8.9','abc','0.9.4-rc.1','v0.9.4','0.9','0.9.3.1','',nil} do
      assert.is_false(C.herdr_version_supported(version),tostring(version))
    end
  end)
end)

describe('selected Herdr among several installed releases',function()
  it('selects the highest release regardless of PATH order',function()
    local run=installed({['/a/herdr']={'0.9.3',22},['/b/herdr']={'0.10.0',22},['/c/herdr']={'0.9.4',22}},{})
    assert.are.same({path='/b/herdr',release='0.10.0'},C.select_herdr({'/a/herdr','/b/herdr','/c/herdr'},run))
  end)
  it('falls back to the highest compatible release when a newer one is not protocol 22',function()
    local calls={}
    local run=installed({['/a/herdr']={'0.9.3',22},['/b/herdr']={'0.10.0',23},['/c/herdr']={'0.9.4',22}},calls)
    assert.are.same({path='/c/herdr',release='0.9.4'},C.select_herdr({'/a/herdr','/b/herdr','/c/herdr'},run))
    assert.is_true(#calls>0)
  end)
  it('keeps PATH order between equal releases',function()
    local run=installed({['/a/herdr']={'0.9.4',22},['/b/herdr']={'0.9.4',22}},{})
    assert.are.equal('/a/herdr',C.select_herdr({'/a/herdr','/b/herdr'},run).path)
  end)
  it('reports every rejected candidate when none is compatible',function()
    local run=installed({['/a/herdr']={'0.9.2',22},['/b/herdr']={'0.10.0',23}},{})
    local err=error_of(function() C.select_herdr({'/a/herdr','/b/herdr','/missing/herdr'},run) end)
    assert.are.same({'/a/herdr','/b/herdr','/missing/herdr'},(function()
      local paths={}; for _,entry in ipairs(err.data.candidates) do paths[#paths+1]=entry.path end; return paths
    end)())
    assert.is_truthy(err.message:find('/b/herdr (0.10.0: bundled schema is not protocol 22)',1,true))
    assert.is_truthy(err.message:find('/a/herdr (unknown: not a herdr X.Y.Z >= 0.9.3)',1,true))
    local empty=error_of(function() C.select_herdr({},installed({},{})) end)
    assert.is_truthy(empty.message:find('Candidates on PATH: none',1,true))
  end)
  it('probes version before the bundled schema without creating resources',function()
    local calls={}
    local run=function(argv) calls[#calls+1]=table.concat(argv,' '); return {exit=0,out=(argv[2]=='--version' and 'herdr 0.9.3' or '{"$schema":"fixture","protocol":22,"schema_version":1}')} end
    assert.are.same({path='/x/herdr',release='0.9.3'},C.select_herdr({'/x/herdr'},run))
    assert.are.same({'/x/herdr --version','/x/herdr api schema --json'},calls)
  end)
  it('lists distinct herdr executables in PATH order',function()
    local existing={a='a',b='b'}
    local fs=function(path)
      local match=path:match('[\\/]([ab])[\\/]herdr%.exe$')
      if not match then return nil end
      return {executable=true,canonical='/'..existing[match]..'/herdr'}
    end
    assert.are.same({'/b/herdr','/a/herdr'},C.herdr_candidates('Windows 11','"'..'/b'..'";/none;;/a;/b',fs))
    assert.are.same({},C.herdr_candidates('Mac OS X','/a:/b',fs))
  end)
end)

describe('Herdr schema failures',function()
  it('rejects missing malformed or failed bundled schema without further commands',function()
    local schemas={
      {exit=nil,out=''},
      {exit=1,out='{"$schema":"fixture","protocol":22,'},
      {exit=0,out='not json'},
      {exit=0,out='{"$schema":"fixture","protocol":21,"schema_version":1}'},
    }
    for _,schema in ipairs(schemas) do
      local calls={}
      local run=function(argv)
        calls[#calls+1]=argv
        if argv[2]=='--version' then return {exit=0,out='herdr 0.9.3'} end
        return schema
      end
      local err=error_of(function() C.select_herdr({'/x/herdr'},run) end)
      assert.is_truthy(err)
      assert.are.equal(2,#calls)
      assert.are.same({'/x/herdr','--version'},calls[1])
      assert.are.same({'/x/herdr','api','schema','--json'},calls[2])
    end
  end)
end)

describe('companion selection argv',function()
  it('removes only the exact lowercase backend option and retains other argv',function()
    assert.are.same({args={'--BACKEND','custom.edn','--restart'},backend='herdr'},
      C.selection_args{'--BACKEND','custom.edn','--backend','herdr','--restart'})
  end)
  it('refuses retired, unknown and duplicate backend options',function()
    for _,args in ipairs{{'--backend','tmux'},{'--backend','psmux'}} do
      local err=error_of(function() C.selection_args(args) end)
      assert.are.equal('retired-backend',err.data.reason)
    end
    for _,args in ipairs{{'--backend','invalid'},{'--backend'},{'--backend','herdr','--backend','herdr'}} do
      local err=error_of(function() C.selection_args(args) end)
      assert.is_truthy(err.message:find('Unknown or duplicate companion backend',1,true))
    end
  end)
end)

describe('read-only companion preflight',function()
  local function preflight(record,opts)
    opts=opts or {}
    return C.preflight_checks(record,{root=opts.root or 'C:/project',requested=opts.requested,
      restart=opts.restart,os=opts.os or 'Windows',canonical=function(p) return (p:gsub('\\','/')) end})
  end
  it('accepts a fresh launch and a complete verified Herdr ownership',function()
    assert.are.equal('herdr',preflight(nil))
    assert.are.equal('herdr',preflight(map{backend=kw('herdr'),['owner-role']=kw('companion'),
      cwd='C:/project',session='uml-viewer-A',pane='w1:p1',socket='C:/s/herdr.sock',workspace='w1',
      ['terminal-id']='term_A',['shell-pid']=42,['owner-start']='ss',['server-pid']=41,['server-start']='rs'}))
  end)
  it('refuses explicit and unmarked legacy ownership without changing bytes',function()
    for _,backend in ipairs{kw('tmux'),kw('psmux'),nil} do
      for _,restart in ipairs{false,true} do
        local record=map{cwd='C:/project',session='saved',pane='%1'}
        if backend then G.put(record,'backend',backend) end
        local err=error_of(function() preflight(record,{restart=restart}) end)
        assert.are.equal('legacy-ownership',err.data.reason)
        assert.is_truthy(err.message:find('old release',1,true))
        assert.is_truthy(err.message:find('Do not use --restart to migrate',1,true))
      end
    end
  end)
  it('refuses unknown recorded backends, retired requests and unsupported platforms',function()
    local unknown=error_of(function() preflight(map{backend=kw('other'),['owner-role']=kw('companion')}) end)
    assert.is_truthy(unknown.message:find('Unknown recorded companion backend',1,true))
    for _,backend in ipairs{'tmux','psmux','unknown'} do
      local err=error_of(function() preflight(nil,{requested=backend}) end)
      assert.is_truthy(err.message:find('companions are retired',1,true))
    end
    local linux=error_of(function() preflight(nil,{os='Linux'}) end)
    assert.is_truthy(linux.message:find('Linux/WSL companion launch is unsupported',1,true))
  end)
  it('rejects missing restart, corrupt records and incomplete or foreign ownership',function()
    local missing=error_of(function() preflight(nil,{restart=true}) end)
    assert.is_truthy(missing.message:find('ownership is missing',1,true))
    local corrupt=error_of(function() preflight(E.vector{}) end)
    assert.is_truthy(corrupt.message:find('corrupt or unreadable',1,true))
    local valid=map{backend=kw('herdr'),['owner-role']=kw('companion'),cwd='C:/project',session='saved',pane='%1',
      socket='C:/s',workspace='w1',['terminal-id']='t',['shell-pid']=1,['owner-start']='o',
      ['server-pid']=2,['server-start']='r'}
    for _,change in ipairs{
      {provisional=true},
      {['terminal-id']=E.nil_value},
      {cwd='C:/other'},
      {['owner-role']=kw('launcher')},
      {session=''},
    } do
      local record=G.copy(valid)
      for key,value in pairs(change) do
        if value==E.nil_value then
          local kept=G.copy(E.map())
          for _,entry in ipairs(record) do if G.name(entry[1])~=key then kept[#kept+1]={entry[1],entry[2]} end end
          record=kept
        else G.put(record,key,value) end
      end
      local err=error_of(function() preflight(record,{restart=true}) end)
      assert.is_truthy(err.message:find('incomplete, provisional or belongs to another project',1,true))
    end
  end)
end)
