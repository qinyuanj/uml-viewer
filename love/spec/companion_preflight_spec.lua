local lfs=require('lfs')
local Ownership=require('uml.adapters.ownership')
local Preflight=require('uml.adapters.companion_preflight')

local function temp_root()
  local name=os.tmpname()
  os.remove(name)
  assert(lfs.mkdir(name))
  assert(lfs.mkdir(name..'/.uml-viewer'))
  return name:gsub('\\','/')
end
local function remove_root(root)
  for name in lfs.dir(root..'/.uml-viewer') do
    if name~='.' and name~='..' then os.remove(root..'/.uml-viewer/'..name) end
  end
  lfs.rmdir(root..'/.uml-viewer')
  lfs.rmdir(root)
end
local function write_record(root,text)
  local file=assert(io.open(root..'/.uml-viewer/companion.edn','wb'))
  file:write(text); file:close()
end
local function installed(calls)
  return function(argv)
    calls[#calls+1]=argv
    if argv[2]=='--version' then return {exit=0,out='herdr 0.9.4'} end
    return {exit=0,out='{"$schema":"fixture","protocol":22,"schema_version":1}'}
  end
end
local function probe(path) return {executable=true,canonical=path:gsub('\\','/')} end
local function select(root,args,options)
  options=options or {}
  options.os=options.os or 'Linux'
  options.path=options.path or '/tools'
  options.probe=options.probe or probe
  options.run=options.run or installed(options.calls or {})
  return Preflight.select(root,args,options)
end
local function failure(root,args,options)
  local ok,err=pcall(select,root,args,options)
  assert.is_false(ok,'expected preflight failure')
  return err
end

describe('read-only companion preflight adapter',function()
  it('defaults fresh launches to selected Herdr without creating ownership or resources',function()
    local root=temp_root()
    local calls={}
    local result=select(root,{'--backend','herdr','diagram with spaces.edn'},{calls=calls})
    assert.are.equal('herdr',result.backend)
    assert.are.same({'diagram with spaces.edn'},result.args)
    assert.are.equal('/tools/herdr',result.selected.path)
    assert.are.same({{'/tools/herdr','--version'},{'/tools/herdr','api','schema','--json'}},calls)
    assert.is_false(Ownership.exists(root))
    remove_root(root)
  end)
  it('treats uppercase option-like argv literally',function()
    local root=temp_root()
    local result=select(root,{'--BACKEND','custom.edn','--backend','herdr'})
    assert.are.same({'--BACKEND','custom.edn'},result.args)
    remove_root(root)
  end)
  it('refuses legacy, unmarked, unknown and incomplete ownership before any command',function()
    local records={
      '{:backend :tmux :cwd "%s" :session "saved" :pane "%%1"}',
      '{:cwd "%s" :session "saved" :pane "%%1"}',
      '{:backend :unknown :cwd "%s" :session "saved" :pane "%%1"}',
      '{:backend :herdr :owner-role :companion :cwd "%s" :session "saved" :pane "%%1"}',
      '{:backend :herdr :owner-role :launcher :cwd "%s" :session "saved" :pane "%%1"}',
      '{:backend :herdr :owner-role :companion :cwd "%s" :session "saved" :pane "%%1" :provisional true}',
    }
    for _,pattern in ipairs(records) do
      local root=temp_root()
      write_record(root,string.format(pattern,root))
      local before=Ownership.bytes(root)
      local calls={}
      local err=failure(root,{}, {calls=calls})
      assert.is_truthy(err~=nil)
      assert.are.equal(0,#calls)
      assert.are.equal(before,Ownership.bytes(root))
      remove_root(root)
    end
  end)
  it('retains corrupt ownership bytes and refuses a missing restart',function()
    local root=temp_root()
    write_record(root,'[:invalid]')
    local calls={}
    local err=failure(root,{}, {calls=calls})
    assert.is_truthy(tostring(err):find('corrupt or unreadable',1,true) or tostring(err):find('incomplete',1,true))
    assert.are.equal(0,#calls)
    os.remove(root..'/.uml-viewer/companion.edn')
    local missing=failure(root,{'--restart'},{restart=true})
    assert.is_truthy(tostring(missing):find('ownership is missing',1,true))
    remove_root(root)
  end)
  it('selects Herdr on WSL',function()
    local root=temp_root()
    local calls={}
    local result=select(root,{'--backend','herdr'},{os='Linux',calls=calls,path='/usr/bin'})
    assert.are.equal('herdr',result.backend)
    assert.are.equal('herdr',result.selected.release and 'herdr' or nil)
    remove_root(root)
  end)
  it('refuses macOS, which is not committed, before touching PATH',function()
    local root=temp_root()
    local calls={}
    local err=failure(root,{}, {os=Preflight.host_os({getOS=function() return 'OS X' end}),calls=calls})
    assert.is_truthy(tostring(err):find('Unsupported companion platform: OS X',1,true))
    assert.are.equal(0,#calls)
    remove_root(root)
  end)
  it('reports the host as LOVE names it, and Linux when LOVE is absent',function()
    assert.are.equal('Linux',Preflight.host_os({getOS=function() return 'Linux' end}))
    assert.are.equal('Windows',Preflight.host_os({getOS=function() return 'Windows' end}))
    assert.are.equal('Linux',Preflight.host_os(nil))
  end)
  it('refuses Windows native before touching PATH',function()
    local root=temp_root()
    local calls={}
    local err=failure(root,{}, {os='Windows',calls=calls})
    assert.is_truthy(tostring(err):find('not a supported platform',1,true))
    assert.are.equal(0,#calls)
    remove_root(root)
  end)

  it('refuses a platform it cannot launch a companion on',function()
    local root=temp_root()
    local calls={}
    local err=failure(root,{}, {os='Plan9',calls=calls})
    assert.is_truthy(tostring(err):find('Unsupported companion platform',1,true))
    assert.are.equal(0,#calls)
    remove_root(root)
  end)
  it('reports every incompatible PATH candidate',function()
    local root=temp_root()
    local calls={}
    local err=failure(root,{}, {calls=calls,path='/a:/b',run=function(argv)
      calls[#calls+1]=argv
      if argv[2]=='--version' then return {exit=0,out=(argv[1]:find('/a/') and 'herdr 0.9.2' or 'herdr 0.10.0')} end
      return {exit=0,out='{"$schema":"fixture","protocol":23,"schema_version":1}'}
    end})
    assert.is_truthy(tostring(err):find('Candidates on PATH',1,true))
    assert.is_truthy(tostring(err):find('/a/herdr (unknown: not a herdr X.Y.Z >= 0.9.3)',1,true))
    assert.is_truthy(tostring(err):find('/b/herdr (0.10.0: bundled schema is not protocol 22)',1,true))
    remove_root(root)
  end)
end)
