local lfs=require('lfs')
local E=require('uml.edn')
local Recovery=require('uml.adapters.recovery')
local R=require('uml.domain.recovery')

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
local function fast_write(path,state)
  local file=assert(io.open(path,'wb'))
  file:write(E.encode(Recovery.to_edn(state))); file:close()
  return state
end
local IO={write_state=fast_write}
local function supervisor(root)
  return {token='controller',
    manifest=root..'/.uml-viewer/recovery-launch.edn',
    state_file=root..'/.uml-viewer/recovery.edn',
    control_file=root..'/.uml-viewer/recovery-control.edn',
    gate_file=root..'/.uml-viewer/recovery-gate.lock',
    lock_file=root..'/.uml-viewer/recovery-controller.lock'}
end
local function fixture()
  local root=temp_root()
  local identity={cwd=root,session='uml-viewer-A',runner_token='initial',
    runner_state=root..'/.uml-viewer/runner.edn',supervisor=supervisor(root)}
  Recovery.register(identity,{pid=10,start='start'},IO)
  return {root=root,identity=identity}
end
local function exit_now(identity,code)
  local state=Recovery.status(identity)
  fast_write(state.runner_state,{token=state.runner_token,phase='exited',
    child={pid=20,start='child'},exit=code})
end
local function child_owner(child)
  return function(pid) if child and tonumber(pid)==child.pid then return {pid=child.pid,start=child.start} end end
end

describe('project recovery controller contract',function()
  it('persists a finite first retry deadline for a verified abnormal exit',function()
    local root=temp_root()
    local identity={cwd=root,session='uml-viewer-A',runner_token='initial',
      runner_state=root..'/.uml-viewer/runner.edn',supervisor=supervisor(root)}
    local launched={}
    Recovery.register(identity,{pid=10,start='start'},IO)
    fast_write(identity.runner_state,{token='initial',phase='exited',child={pid=20,start='child'},exit=37})
    Recovery.step(identity,1000,child_owner(nil),function(value) launched[#launched+1]=value end,IO)
    assert.are.equal('backoff',Recovery.status(identity).phase)
    assert.are.equal(2000,Recovery.status(identity).deadline)
    assert.are.equal(0,Recovery.status(identity).attempts)
    assert.are.equal(0,#launched)
    Recovery.step(identity,1999,child_owner(nil),function(value) launched[#launched+1]=value end,IO)
    assert.are.equal(0,#launched)
    Recovery.step(identity,2000,child_owner(nil),function(value) launched[#launched+1]=value end,IO)
    assert.are.equal(1,#launched)
    assert.are.equal(1,Recovery.status(identity).attempts)
    assert.are.equal(1,Recovery.status(identity).generation)
    assert.are_not.equal('initial',launched[1].runner_token)
    remove_root(root)
  end)
  it('exhausts exactly three retries at one, two and four seconds',function()
    local f=fixture(); local launched={}
    for _,case in ipairs{{0,1000,1},{1100,3100,2},{3200,7200,3}} do
      exit_now(f.identity,37)
      Recovery.step(f.identity,case[1],child_owner(nil),function(v) launched[#launched+1]=v end,IO)
      assert.are.equal(case[2],Recovery.status(f.identity).deadline)
      Recovery.step(f.identity,case[2]-1,child_owner(nil),function(v) launched[#launched+1]=v end,IO)
      assert.are.equal(case[3]-1,#launched)
      Recovery.step(f.identity,case[2],child_owner(nil),function(v) launched[#launched+1]=v end,IO)
      assert.are.equal(case[3],#launched)
    end
    exit_now(f.identity,37)
    Recovery.step(f.identity,7300,child_owner(nil),function(v) launched[#launched+1]=v end,IO)
    assert.are.equal('failed',Recovery.status(f.identity).phase)
    assert.are.equal('Recovery exhausted after three retries',Recovery.status(f.identity).reason)
    Recovery.step(f.identity,100000,child_owner(nil),function(v) launched[#launched+1]=v end,IO)
    assert.are.equal(3,#launched)
    remove_root(f.root)
  end)
  it('resets only after sixty seconds of verified continuous child uptime',function()
    local f=fixture(); local child={pid=20,start='child'}
    exit_now(f.identity,37)
    Recovery.step(f.identity,0,child_owner(nil),function() end,IO)
    Recovery.step(f.identity,1000,child_owner(nil),function() end,IO)
    local state=Recovery.status(f.identity)
    fast_write(state.runner_state,{token=state.runner_token,phase='started',child=child})
    Recovery.step(f.identity,1100,child_owner(child),function() end,IO)
    Recovery.step(f.identity,61099,child_owner(child),function() end,IO)
    assert.are.equal(1,Recovery.status(f.identity).attempts)
    Recovery.step(f.identity,61100,child_owner(child),function() end,IO)
    assert.are.equal(0,Recovery.status(f.identity).attempts)
    exit_now(f.identity,37)
    Recovery.step(f.identity,61200,child_owner(nil),function() end,IO)
    assert.are.equal(62200,Recovery.status(f.identity).deadline)
    remove_root(f.root)
  end)
  it('never recovers normal, stale, unknown or unverifiable exits',function()
    for _,event in ipairs{
      {token='initial',phase='exited',exit=0,child={pid=20,start='child'}},
      {token='stale',phase='exited',exit=37,child={pid=20,start='child'}},
      {token='initial',phase='exited',exit=37},
    } do
      local f=fixture(); local launched={}
      fast_write(f.identity.runner_state,event)
      Recovery.step(f.identity,0,child_owner(nil),function(v) launched[#launched+1]=v end,IO)
      assert.is_true(Recovery.status(f.identity).phase=='normal' or Recovery.status(f.identity).phase=='failed')
      Recovery.step(f.identity,99999,child_owner(nil),function(v) launched[#launched+1]=v end,IO)
      assert.are.equal(0,#launched)
      remove_root(f.root)
    end
  end)
  it('allows a bounded grace for the runner to record a child exit',function()
    local f=fixture(); local child={pid=20,start='child'}
    fast_write(f.identity.runner_state,{token='initial',phase='started',child=child})
    Recovery.step(f.identity,0,child_owner(nil),function() end,IO)
    assert.are.equal('running',Recovery.status(f.identity).phase)
    assert.are.equal(0,Recovery.status(f.identity).child_gone_since)
    fast_write(f.identity.runner_state,{token='initial',phase='exited',child=child,exit=37})
    Recovery.step(f.identity,100,child_owner(nil),function() end,IO)
    assert.are.equal('backoff',Recovery.status(f.identity).phase)
    assert.are.equal(1100,Recovery.status(f.identity).deadline)
    remove_root(f.root)
  end)
  it('fails only when the exit record never arrives within the grace',function()
    local f=fixture(); local child={pid=20,start='child'}
    fast_write(f.identity.runner_state,{token='initial',phase='started',child=child})
    Recovery.step(f.identity,1000,child_owner(nil),function() end,IO)
    Recovery.step(f.identity,2999,child_owner(nil),function() end,IO)
    Recovery.step(f.identity,3000,child_owner(nil),function() end,IO)
    assert.are.equal('running',Recovery.status(f.identity).phase)
    Recovery.step(f.identity,3001,child_owner(nil),function() end,IO)
    assert.are.equal('failed',Recovery.status(f.identity).phase)
    assert.are.equal('Runner child disappeared without a matching exit; recovery unsupported',
      Recovery.status(f.identity).reason)
    remove_root(f.root)
  end)
  it('clears the grace marker when the child is verified alive again',function()
    local f=fixture(); local child={pid=20,start='child'}
    fast_write(f.identity.runner_state,{token='initial',phase='started',child=child})
    Recovery.step(f.identity,0,child_owner(nil),function() end,IO)
    assert.are.equal(0,Recovery.status(f.identity).child_gone_since)
    Recovery.step(f.identity,100,child_owner(child),function() end,IO)
    assert.is_nil(Recovery.status(f.identity).child_gone_since)
    assert.are.equal('running',Recovery.status(f.identity).phase)
    remove_root(f.root)
  end)
  it('durable cancellation beats a due retry and never affects another project',function()
    local a=fixture(); local b=fixture(); local launched={}
    exit_now(a.identity,37); Recovery.step(a.identity,0,child_owner(nil),function(v) launched[#launched+1]=v end,IO)
    exit_now(b.identity,37); Recovery.step(b.identity,0,child_owner(nil),function(v) launched[#launched+1]=v end,IO)
    fast_write(a.identity.supervisor.control_file,{cancel='controller'})
    Recovery.step(a.identity,1000,child_owner(nil),function(v) launched[#launched+1]=v end,IO)
    assert.are.equal('cancelled',Recovery.status(a.identity).phase)
    assert.are.equal('backoff',Recovery.status(b.identity).phase)
    assert.are.equal(0,#launched)
    remove_root(a.root); remove_root(b.root)
  end)
  it('verifies only the exact original controller identity',function()
    local f=fixture()
    assert.are.equal('running',Recovery.verify(f.identity,function(pid)
      if pid==10 then return {pid=10,start='start'} end end).phase)
    for _,controller in ipairs{{pid=11,start='start'},{pid=10,start='other'},{start='start'}} do
      local state=Recovery.status(f.identity); state.controller=controller
      fast_write(f.identity.supervisor.state_file,state)
      local ok=pcall(Recovery.verify,f.identity,function(pid)
        if pid==10 then return {pid=10,start='start'} end end)
      assert.is_false(ok)
    end
    remove_root(f.root)
  end)
  it('does not replay an uncertain submission after controller rebind',function()
    local f=fixture(); local launches=0
    exit_now(f.identity,37)
    Recovery.step(f.identity,0,child_owner(nil),function() end,IO)
    Recovery.step(f.identity,1000,child_owner(nil),function()
      launches=launches+1; error('timeout',0)
    end,IO)
    assert.are.equal('failed',Recovery.status(f.identity).phase)
    assert.are.equal(1,Recovery.status(f.identity).attempts)
    Recovery.step(f.identity,999999,child_owner(nil),function() launches=launches+1 end,IO)
    assert.are.equal(1,launches)
    remove_root(f.root)
  end)
end)

describe('recovery journal IO',function()
  it('retries a transient read failure while the controller replaces the journal',function()
    local root=temp_root()
    local target=root..'/.uml-viewer/state.edn'
    fast_write(target,{phase='running'})
    local calls=0
    local value=Recovery.read_state(target,{open=function(path)
      calls=calls+1
      if calls<3 then return nil,'sharing violation' end
      return io.open(path,'rb')
    end,sleep=function() end})
    assert.are.equal('running',value.phase)
    assert.are.equal(3,calls)
    remove_root(root)
  end)
  it('still fails when the violation outlasts the retry budget',function()
    local root=temp_root()
    local target=root..'/.uml-viewer/state.edn'
    fast_write(target,{phase='running'})
    local now=0
    local ok=pcall(Recovery.read_state,target,{open=function() return nil,'sharing violation' end,
      clock=function() return now end, sleep=function(ms) now=now+ms end})
    assert.is_false(ok)
    remove_root(root)
  end)
  it('does not retry a malformed journal',function()
    local root=temp_root()
    local target=root..'/.uml-viewer/state.edn'
    local file=assert(io.open(target,'wb')); file:write('{:phase'); file:close()
    local ok=pcall(Recovery.read_state,target)
    assert.is_false(ok)
    remove_root(root)
  end)
  it('does not rewrite the journal when a step changes nothing',function()
    local f=fixture()
    local writes=0
    local write=fast_write
    local options={write_state=function(path,state) writes=writes+1; write(path,state) end}
    Recovery.step(f.identity,0,child_owner(nil),function() end,IO,options)
    local first=writes
    Recovery.step(f.identity,50,child_owner(nil),function() end,IO,options)
    assert.are.equal(first,writes)
    remove_root(f.root)
  end)
  it('retires completed lifecycle journals but never a live one',function()
    for _,phase in ipairs{'cancelled','failed','normal'} do
      local f=fixture()
      local state=Recovery.status(f.identity); state.phase=phase
      fast_write(f.identity.supervisor.state_file,state)
      fast_write(f.identity.supervisor.control_file,{cancel='controller'})
      assert.is_true(Recovery.retire_completed(f.identity))
      assert.is_nil(Recovery.status(f.identity))
      remove_root(f.root)
    end
    for _,phase in ipairs{'running','backoff','submitting'} do
      local f=fixture()
      local state=Recovery.status(f.identity); state.phase=phase
      fast_write(f.identity.supervisor.state_file,state)
      assert.is_false(pcall(Recovery.retire_completed,f.identity))
      assert.are.equal(phase,Recovery.status(f.identity).phase)
      remove_root(f.root)
    end
  end)
  it('leaves an absent journal alone',function()
    local root=temp_root()
    local identity={cwd=root,session='uml-viewer-A',runner_token='initial',
      runner_state=root..'/.uml-viewer/runner.edn',supervisor=supervisor(root)}
    assert.is_nil(Recovery.retire_completed(identity))
    local names={}
    for name in lfs.dir(root..'/.uml-viewer') do
      if name~='.' and name~='..' then names[#names+1]=name end
    end
    assert.are.same({},names)
    assert.is_nil(io.open(identity.supervisor.state_file,'rb'))
    assert.is_nil(io.open(identity.supervisor.control_file,'rb'))
    remove_root(root)
  end)
  it('coerces JVM keyword phases and journals into the same state model',function()
    local root=temp_root()
    local target=root..'/.uml-viewer/recovery.edn'
    local file=assert(io.open(target,'wb'))
    file:write('{:token "controller" :session "uml-viewer-A" :cwd "'..root
      ..'" :controller {:pid 10 :start "start"} :phase :running :generation 2 :attempts 1}')
    file:close()
    local value=Recovery.read_state(target)
    assert.are.equal('running',value.phase)
    assert.are.equal(2,value.generation)
    assert.are.equal(10,value.controller.pid)
    remove_root(root)
  end)
end)
