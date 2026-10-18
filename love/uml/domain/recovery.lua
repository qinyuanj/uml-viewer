-- Pure durable recovery policy. The controller is the only journal writer;
-- retries are bounded and only a verified abnormal child exit qualifies.
local M={}
M.backoff={1000,2000,4000}
M.reset_ms=60000
M.gone_grace_ms=2000

local function copy(value)
  if type(value)~='table' then return value end
  local out={}
  for key,item in pairs(value) do out[key]=copy(item) end
  return out
end
M.copy=copy

function M.same(a,b)
  if type(a)~='table' or type(b)~='table' then return a==b end
  for key,item in pairs(a) do if not M.same(item,b[key]) then return false end end
  for key in pairs(b) do if a[key]==nil then return false end end
  return true
end

function M.reference(identity)
  if identity.supervisor then return identity.supervisor end
  local root=(identity.cwd or '.'):gsub('\\','/')..'/.uml-viewer'
  return {token='controller',
    manifest=root..'/recovery-launch.edn',state_file=root..'/recovery.edn',
    control_file=root..'/recovery-control.edn',gate_file=root..'/recovery-gate.lock',
    lock_file=root..'/recovery-controller.lock'}
end

function M.matching(identity,state)
  local ref=M.reference(identity)
  if not state or ref.token~=state.token or identity.cwd~=state.cwd or identity.session~=state.session then
    error('Recovery controller identity is missing or changed',0)
  end
  return state
end

function M.terminal(phase)
  return phase=='failed' or phase=='cancelled' or phase=='normal'
end

function M.initial(identity,controller,now)
  local ref=M.reference(identity)
  return {token=ref.token,session=identity.session,cwd=identity.cwd,controller=controller,
    phase='running',generation=0,attempts=0,submitted_at=now,
    runner_token=identity.runner_token,runner_state=identity.runner_state}
end

local function same_child(a,b)
  return type(a)=='table' and type(b)=='table' and a.pid==b.pid and a.start==b.start
end

local function child_present(child,owner)
  if type(child)~='table' then return false end
  local observed=owner(child.pid)
  return observed~=nil and same_child(child,observed)
end

local function child_absent(child,owner)
  return type(child)=='table' and type(child.pid)=='number' and child.pid==math.floor(child.pid)
    and child.pid>0 and type(child.start)=='string' and owner(child.pid)==nil
end

local function stopped(state,reason)
  local result=copy(state); result.phase='failed'; result.reason=reason; return result
end

-- ctx: identity, state, control, event, now, owner, token(), runner_state(generation)
-- Returns state, changed, launch.
function M.decide(ctx)
  local state=M.matching(ctx.identity,ctx.state)
  local ref=M.reference(ctx.identity)
  local control=ctx.control or {}
  local now=ctx.now
  if control.cancel==ref.token then
    local result=copy(state); result.phase='cancelled'; result.reason='Viewer closed; recovery revoked'
    return result,true,false
  end
  if M.terminal(state.phase) then return state,false,false end
  if state.phase=='backoff' then
    if now<state.deadline then return state,false,false end
    local generation=(state.generation or 0)+1
    local pending=copy(state)
    pending.phase='submitting'; pending.generation=generation; pending.attempts=(state.attempts or 0)+1
    pending.runner_token=ctx.token()
    pending.runner_state=ctx.runner_state(generation)
    pending.submitted_at=now
    pending.deadline=nil; pending.uptime_since=nil; pending.child=nil
    return pending,true,true
  end
  if state.phase=='submitting' then
    return stopped(state,'Unacknowledged recovery submission; refusing duplicate'),true,false
  end
  local event=ctx.event
  if event==nil then
    if now-(state.submitted_at or now)>30000 then
      return stopped(state,'Runner acknowledgement missing; exit cause unknown'),true,false
    end
    return state,false,false
  end
  if state.runner_token~=event.token then
    return stopped(state,'Runner generation changed; exit cause unknown'),true,false
  end
  if event.phase=='started' then
    local child=event.child
    if child_present(child,ctx.owner) then
      local result=copy(state)
      local since=state.uptime_since or now
      result.child=copy(child); result.uptime_since=since; result.child_gone_since=nil
      if now-since>=M.reset_ms then result.attempts=0 end
      return result,not M.same(result,state),false
    end
    local gone_since=state.child_gone_since or now
    if now-gone_since>M.gone_grace_ms then
      return stopped(state,'Runner child disappeared without a matching exit; recovery unsupported'),true,false
    end
    local result=copy(state); result.child_gone_since=gone_since
    return result,not M.same(result,state),false
  end
  if event.phase=='exited' then
    local exit=event.exit
    local valid=type(exit)=='number' and exit==math.floor(exit) and child_absent(event.child,ctx.owner)
      and (state.child==nil or M.same(event.child,state.child))
    if not valid then
      return stopped(state,'Unverified child exit; recovery unsupported'),true,false
    end
    if exit==0 then
      local result=copy(state); result.phase='normal'; result.child=copy(event.child); result.exit=0
      result.reason='Companion exited normally'
      return result,true,false
    end
    if (state.attempts or 0)>=#M.backoff then
      local result=stopped(state,'Recovery exhausted after three retries'); result.exit=exit
      return result,true,false
    end
    local result=copy(state)
    result.phase='backoff'; result.child=copy(event.child); result.exit=exit
    result.deadline=now+M.backoff[(state.attempts or 0)+1]
    return result,true,false
  end
  return stopped(state,'Runner exit cause unknown; recovery unsupported'),true,false
end

return M
