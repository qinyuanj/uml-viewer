local lfs=require('lfs')
local E=require('uml.edn')
local Ownership=require('uml.adapters.ownership')
local Lifecycle=require('uml.adapters.companion_lifecycle')
local Claude=require('uml.adapters.claude')

local function temp_root()
  local name=os.tmpname(); os.remove(name)
  assert(lfs.mkdir(name)); assert(lfs.mkdir(name..'/.uml-viewer'))
  return name:gsub('\\','/')
end
local function remove_root(root)
  for name in lfs.dir(root..'/.uml-viewer') do
    if name~='.' and name~='..' then os.remove(root..'/.uml-viewer/'..name) end
  end
  lfs.rmdir(root..'/.uml-viewer'); lfs.rmdir(root)
end
local function raw_write(root,text)
  local file=assert(io.open(root..'/.uml-viewer/companion.edn','wb')); file:write(text); file:close()
end
local function raw_read(root)
  local file=io.open(root..'/.uml-viewer/companion.edn','rb'); if not file then return nil end
  local text=file:read('*a'); file:close(); return text
end
local function options(run_calls)
  local calls=run_calls or {}
  return {canonical=function(path) return (path or ''):gsub('\\','/') end,
    preflight={os='Linux',path='/tools',probe=function(path) return {executable=true,canonical=path} end,
      run=function(argv) calls[#calls+1]=argv
        if argv[2]=='--version' then return {exit=0,out='herdr 0.9.4'} end
        return {exit=0,out='{"$schema":"x","protocol":22,"schema_version":1}'} end},
    claude=function() return '/Tools/claude' end,
    uuid=function() return '12345678-uuid' end}
end
local function fake_backend(captured)
  return {create=function(session,cwd,command)
      captured.session=session; captured.cwd=cwd; captured.command=command
      return {backend='herdr',owner_role='companion',cwd=cwd,session=session,pane='w1:p1',
        socket='C:/s',workspace='w1',terminal_id='t',shell_pid=1,owner_start='o',
        server_pid='2',server_start='r',command=command}
    end,
    cleanup=function(session) captured.cleaned=session; return {released=true} end}
end
local function minimal_owned(root)
  return '{:backend :herdr :owner-role :companion :cwd "'..root..'" :session "uml-viewer-a" '
    ..':socket "C:/s" :workspace "w1" :pane "w1:p1" :terminal-id "t" :shell-pid 1 '
    ..':owner-start "o" :server-pid "2" :server-start "r" :command ["claude.exe"]}'
end
describe('companion lifecycle ownership',function()
  it('names a fresh session uniquely per launch within the Herdr grammar',function()
    local name=Lifecycle.session_name('C:/projects/some project','abcdef12-rest')
    assert.is_true(#name<=64)
    local long=Lifecycle.session_name('C:/projects/'..string.rep('x',200),'abcdef12-rest')
    assert.is_true(#long<=64)
    assert.is_truthy(long:match('^uml%-viewer%-x+%-%x+%-abcdef12$'))
    -- Two project roots whose names share a prefix must not collide.
    local a=Lifecycle.session_name('C:/projects/alpha-one','abcdef12-rest')
    local b=Lifecycle.session_name('C:/projects/alpha-two','abcdef12-rest')
    assert.are_not.equal(a,b)
  end)
  it('shrinks the name so the Herdr socket path stays inside the measured limit',function()
    -- Herdr 0.9.5 starts its server at 101 bytes but fails workspace creation
    -- from 102 bytes, so the generated name must fit the config root instead of
    -- assuming one. This root is the one the WSL acceptance harness uses.
    local config_root='/tmp/uml-restart-xdg-o7gbqk3s/config'
    local name=Lifecycle.session_name('/projects/some project','abcdef12-rest',config_root)
    local socket=config_root..'/herdr/sessions/'..name..'/herdr.sock'
    assert.is_true(#socket<=Lifecycle.MAX_SOCKET_BYTES,socket..' ('..#socket..' bytes)')
    assert.is_truthy(name:match('^uml%-viewer%-[%w_%-]+$'),name)
    assert.is_true(#name<=Lifecycle.MAX_NAME_BYTES)
    local other=Lifecycle.session_name('/projects/other','abcdef12-rest',config_root)
    assert.are_not.equal(name,other)
    assert.is_truthy(other:match('abcdef12$'))
  end)
  it('reports the real cause when no name can fit the config root',function()
    local error_of=function(fn) local ok,err=pcall(fn); assert.is_false(ok); return err end
    local err=error_of(function()
      Lifecycle.session_name('/projects/some project','abcdef12-rest','/tmp/'..string.rep('x',120))
    end)
    assert.is_truthy(tostring(err):find('XDG_CONFIG_HOME',1,true))
    assert.is_truthy(tostring(err):find('Unix socket path',1,true))
  end)
  it('refuses existing, legacy and corrupt ownership before selecting or creating',function()
    for _,contents in ipairs{minimal_owned('C:/p'),
      '{:backend :tmux :cwd "C:/p" :session "s" :pane "%1"}',
      '{:cwd "C:/p" :session "s" :pane "%1"}',
      '[:invalid]'} do
      local root=temp_root()
      raw_write(root,contents)
      local before=raw_read(root)
      local calls={}
      local opts=options(calls)
      local created=false
      opts.backend=function() created=true; return fake_backend({}) end
      assert.is_truthy(select(2,pcall(Lifecycle.create,root,opts)))
      assert.are.equal(0,#calls)
      assert.is_false(created)
      assert.are.equal(before,raw_read(root))
      remove_root(root)
    end
  end)
  it('creates provisional ownership, the companion and the verbatim launch command',function()
    local root=temp_root()
    local captured={}
    local opts=options()
    opts.backend=function(selected)
      assert.are.equal('/tools/herdr',selected.path)
      return fake_backend(captured)
    end
    local owned=Lifecycle.create(root,opts)
    assert.is_truthy(owned.session:match('^uml%-viewer%-'))
    assert.are.equal('/Tools/claude',captured.command[1])
    assert.are.equal('--session-id',captured.command[3])
    assert.are.equal('--append-system-prompt',captured.command[5])
    assert.are.equal(require('uml.domain.companion').standing_rules,captured.command[6])
    assert.are.equal(root,captured.cwd)
    local record=Ownership.read(root)
    assert.are.equal('herdr',E.kind(E.get(record,E.keyword('backend')))=='keyword'
      and E.get(record,E.keyword('backend')).name or nil)
    remove_root(root)
  end)
  it('retains missing, unknown, foreign and wrong-role ownership without mailbox changes',function()
    local cases={
      nil,
      '{:backend :unknown :cwd "%s" :session "s" :pane "%%1"}',
      minimal_owned('C:/other')..'',
      '{:backend :herdr :owner-role :launcher :cwd "%s" :session "s" :pane "%%1"}',
    }
    for _,pattern in ipairs(cases) do
      local root=temp_root()
      local queue=root..'/.uml-viewer/to-agent.edn'
      local file=assert(io.open(queue,'wb')); file:write('{:next-id 3 :queue [{:id 2 :op :regen}]}'); file:close()
      local queue_before=io.open(queue,'rb'):read('*a')
      if pattern then raw_write(root,string.format(pattern,root)) end
      local calls={}
      local opts=options(calls)
      local ok,err=pcall(Lifecycle.required,root,opts)
      assert.is_false(ok)
      assert.is_truthy(tostring(err):find('missing, ambiguous',1,true)
        or tostring(err):find('old release',1,true)
        or tostring(err):find('Incomplete Herdr ownership',1,true))
      assert.are.equal(0,#calls)
      local queue_after=io.open(queue,'rb'):read('*a')
      assert.are.equal(queue_before,queue_after)
      remove_root(root)
    end
  end)
  it('clears ownership only after confirmed cleanup and retains it on failure',function()
    local root=temp_root()
    raw_write(root,minimal_owned(root))
    local captured={}
    local opts=options()
    opts.backend=function() return fake_backend(captured) end
    assert.are.same({released=true},Lifecycle.shutdown(root,opts))
    assert.are.equal('uml-viewer-a',captured.cleaned)
    assert.are.equal(0,#Ownership.read(root))
    raw_write(root,minimal_owned(root))
    local failed=options()
    failed.backend=function() return {cleanup=function() error('cannot confirm release',0) end} end
    assert.is_false(pcall(Lifecycle.shutdown,root,failed))
    assert.is_not_nil(Ownership.read(root))
    remove_root(root)
  end)
end)

describe('companion terminal attach',function()
  it('reports a manual attach command instead of opening a terminal',function()
    local calls=0
    local opened=Lifecycle.open_window('uml-viewer-owned',{run=function() calls=calls+1 end})
    assert.are.equal('none',opened.terminal)
    assert.are.equal('uml-viewer-owned',opened.session)
    assert.are.equal("'herdr' session attach 'uml-viewer-owned'",opened.attach)
    assert.are.equal(0,calls)
  end)
end)
describe('Claude executable discovery boundary',function()
  it('uses CLAUDE_BIN when it is set and otherwise the bare name',function()
    assert.are.equal('/opt/claude/bin/claude',Claude.executable{env={CLAUDE_BIN='/opt/claude/bin/claude'}})
    assert.are.equal('claude',Claude.executable{env={}})
  end)
end)
