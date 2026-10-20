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
    preflight={os='Windows',path='C:/tools',probe=function(path) return {executable=true,canonical=path} end,
      run=function(argv) calls[#calls+1]=argv
        if argv[2]=='--version' then return {exit=0,out='herdr 0.9.4'} end
        return {exit=0,out='{"$schema":"x","protocol":22,"schema_version":1}'} end},
    claude=function() return 'C:/Tools/claude.exe' end,
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
local alphabet='ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/'
local function decode_utf16le(text)
  local bytes={}
  local buffer,bits=0,0
  for i=1,#text do
    local char=text:sub(i,i)
    if char=='=' then break end
    local value=alphabet:find(char,1,true)
    if value then
      buffer=buffer*64+(value-1); bits=bits+6
      if bits>=8 then
        bits=bits-8
        bytes[#bytes+1]=math.floor(buffer/2^bits)%256
        buffer=buffer%2^bits
      end
    end
  end
  local out={}
  for i=1,#bytes,2 do out[#out+1]=string.char(bytes[i]) end
  return table.concat(out)
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
      assert.are.equal('C:/tools/herdr.exe',selected.path)
      return fake_backend(captured)
    end
    local owned=Lifecycle.create(root,opts)
    assert.is_truthy(owned.session:match('^uml%-viewer%-'))
    assert.are.equal('C:/Tools/claude.exe',captured.command[1])
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

describe('Herdr Windows full session client',function()
  it('opens the Windows full session client with nesting disabled only in its child environment',function()
    local calls={}
    local opened=Lifecycle.open_window('uml-viewer-owned',{
      windows=true,herdr='C:/tools/herdr.exe',
      run=function(argv,timeout,env_remove)
        calls[#calls+1]={argv=argv,timeout=timeout,env_remove=env_remove}
        return {exit=0,out=''}
      end})
    assert.are.equal('windows-terminal',opened.terminal)
    assert.are.equal(1,#calls)
    local call=calls[1]
    assert.are.equal('wt.exe',call.argv[1])
    assert.are.equal('new-tab',call.argv[4])
    assert.are.equal('--title',call.argv[5])
    assert.are.equal('uml-viewer-owned',call.argv[6])
    assert.are.equal('pwsh.exe',call.argv[7])
    assert.are.equal('-EncodedCommand',call.argv[9])
    assert.are.equal(10000,call.timeout)
    assert.are.same({'HERDR_ENV','NO_COLOR'},call.env_remove)
    local script=decode_utf16le(call.argv[10])
    assert.is_truthy(script:find("session attach 'uml-viewer-owned'",1,true))
    assert.is_truthy(script:find("& 'C:/tools/herdr.exe'",1,true))
    assert.is_truthy(script:find('$env:HERDR_ENV=$null',1,true))
    assert.is_truthy(script:find('$env:NO_COLOR=$null',1,true))
  end)
end)
describe('Claude executable discovery boundary',function()
  local Process=require('uml.adapters.process')
  local native_windows
  before_each(function()
    native_windows=Process.windows
    Process.windows=function() return true end
  end)
  after_each(function() Process.windows=native_windows end)
  it('accepts a configured executable with spaces and preserves it literally',function()
    local seen
    local path=Claude.executable{run=function(argv)
      seen=argv
      assert.are.equal('pwsh.exe',argv[1])
      assert.are.equal('-NoProfile',argv[2])
      assert.are.equal('-EncodedCommand',argv[3])
      return {exit=0,out='C:\\With Spaces\\claude.ps1'}
    end}
    assert.are.equal('C:\\With Spaces\\claude.ps1',path)
    assert.is_not_nil(seen)
  end)
  it('fails closed when no safe executable is configured',function()
    assert.is_false(pcall(Claude.executable,{run=function() return {exit=1,out='',err='unsafe wrapper'} end}))
  end)
end)
