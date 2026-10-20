-- Durable project recovery controller boundary. The viewer owns only immutable
-- references; this module is the sole journal writer when running detached.
local E=require('uml.edn')
local Atomic=require('uml.adapters.atomic_file')
local Clock=require('uml.adapters.clock')
local OS=require('uml.adapters.lock')
local Process=require('uml.adapters.process')
local R=require('uml.domain.recovery')
local M={}

-- Existence is checked without opening: an absent journal must not spend the
-- lock budget, while a present-but-locked file must retry. POSIX needs no FFI
-- for this (a successful open is proof). On Windows LuaFileSystem's C module
-- cannot be loaded inside LOVE — it crashes the process rather than raising —
-- so existence there asks kernel32.
local windows=package.config:sub(1,1)=='\\'
local ffi_file_api
local function file_attributes()
  if ffi_file_api~=nil then return ffi_file_api or nil end
  local ok,ffi=pcall(require,'ffi')
  ffi_file_api=false
  if not ok then return nil end
  ffi.cdef[[
    int MultiByteToWideChar(unsigned int,unsigned long,const char*,int,wchar_t*,int);
    unsigned long GetFileAttributesW(const wchar_t*);
  ]]
  ffi_file_api=ffi; return ffi
end
local function exists(path)
  if not windows then
    local file=io.open(path,'rb'); if file then file:close(); return true end; return false
  end
  local ffi=file_attributes()
  if not ffi then
    local file=io.open(path,'rb'); if file then file:close(); return true end; return false
  end
  local count=ffi.C.MultiByteToWideChar(65001,0,path,-1,nil,0)
  local wide=ffi.new('wchar_t[?]',count)
  ffi.C.MultiByteToWideChar(65001,0,path,-1,wide,count)
  return tonumber(ffi.C.GetFileAttributesW(wide))~=4294967295
end

local KEYWORD_VALUES={phase=true,cleanup_phase=true,backend=true,owner_role=true}
local function to_edn(value,key)
  if type(value)~='table' then
    if KEYWORD_VALUES[key] and type(value)=='string' then return E.keyword(value) end
    return value
  end
  if key=='command' or key=='argv' or key=='recovery_command' then
    local vector=E.vector(); for _,item in ipairs(value) do vector[#vector+1]=to_edn(item) end
    return vector
  end
  local map=E.map()
  for name,item in pairs(value) do E.assoc(map,E.keyword(name:gsub('_','-')),to_edn(item,name)) end
  return map
end

local function from_edn(value)
  local kind=E.kind(value)
  if kind=='keyword' or kind=='symbol' then return value.name end
  if kind=='vector' then
    local out={}; for _,item in ipairs(value) do out[#out+1]=from_edn(item) end; return out
  end
  if kind=='map' then
    local out={}
    for _,entry in ipairs(value) do out[entry[1].name:gsub('-','_')]=from_edn(entry[2]) end
    return out
  end
  if kind=='nil' then return nil end
  return value
end
M.to_edn=to_edn; M.from_edn=from_edn

function M.read_state(path,options)
  if not path then return nil end
  options=options or {}
  local clock=options.clock or Clock.monotonic_ms
  local sleep=options.sleep or Clock.sleep
  local open=options.open or io.open
  local deadline=clock()+2000
  while true do
    if not options.open and not exists(path) then return nil end
    handle,message=open(path,'rb')
    if handle then break end
    if options.open and (not message or message:match('[Nn]o such file')) then return nil end
    if clock()<deadline then sleep(25) else
      error('Could not read recovery journal '..path..': '..tostring(message or 'sharing violation'),0)
    end
  end
  local text=handle:read('*a'); handle:close()
  local ok,value=pcall(E.parse,text)
  if not ok then error(value,0) end
  return from_edn(value)
end

-- Only a positively present file is a record; malformed runner state is
-- treated as no acknowledgement, never as an exit cause.
function M.read_runner(state,options)
  local path=state.runner_state
  if not path then return nil end
  local file=io.open(path,'rb'); if not file then return nil end
  local text=file:read('*a'); file:close()
  local ok,value=pcall(E.parse,text)
  if not ok then return nil end
  local event=from_edn(value)
  if event.phase then return event end
  return nil
end

function M.write_state(path,state)
  Atomic.ensure_parent(path)
  Atomic.write(path,E.encode(to_edn(state)))
  return state
end

function M.reference(identity) return R.reference(identity) end
function M.status(identity,options) return M.read_state(M.reference(identity).state_file,options) end
function M.save(identity,state,options)
  local write=(options and options.write_state) or M.write_state
  return write(M.reference(identity).state_file,state)
end

local function identity_owner(owner)
  return function(pid)
    local observed=owner(pid)
    if not observed then return nil end
    return {pid=observed.pid,start=observed.start}
  end
end
function M.default_owner(pid)
  local observed=Process.owner_native(pid)
  if not observed then return nil end
  return {pid=observed.pid,start=observed.start}
end

function M.register(identity,controller,options)
  if M.status(identity,options) then
    error('Recovery journal already exists; refusing replacement',0)
  end
  return M.save(identity,R.initial(identity,controller,(options and options.now) or Clock.monotonic_ms()),options)
end

function M.verify(identity,owner,options)
  owner=identity_owner(owner or M.default_owner)
  local state=M.status(identity,options)
  R.matching(identity,state)
  local controller=state.controller
  if type(controller)~='table' or type(controller.pid)~='number' or controller.pid~=math.floor(controller.pid)
      or controller.pid<1 or type(controller.start)~='string' then
    error('Recovery process identity is incomplete',0)
  end
  if not R.terminal(state.phase) then
    local observed=owner(controller.pid)
    if not observed or observed.pid~=controller.pid or observed.start~=controller.start then
      error('Recovery controller disappeared; refusing replacement',0)
    end
  end
  return state
end

function M.with_gate(identity,fn,options)
  options=options or {}
  local ref=M.reference(identity)
  local acquire=options.acquire or function(path)
    return OS.acquire(path,{sleep=options.sleep or Clock.sleep})
  end
  local release=options.release or OS.release
  local handle=acquire(ref.gate_file)
  local ok,result=pcall(fn)
  release(handle)
  if not ok then error(result,0) end
  return result
end

function M.cancel(identity,options)
  options=options or {}
  local owner=options.owner or M.default_owner
  local clock=options.clock or Clock.monotonic_ms
  local sleep=options.sleep or Clock.sleep
  M.verify(identity,owner,options)
  local ref=M.reference(identity)
  M.with_gate(identity,function()
    local write=options.write_state or M.write_state
    write(ref.control_file,{cancel=ref.token})
  end,options)
  local deadline=clock()+10000
  while true do
    local state=M.verify(identity,owner,options)
    local observed=owner(state.controller.pid)
    -- A reused PID is not the registered controller: only the exact original
    -- creation identity proves that the controller is still running.
    local absent=observed==nil or observed.start~=state.controller.start
    if R.terminal(state.phase) and absent then return {enabled=false} end
    if clock()<deadline then sleep(25) else
      error('Recovery cancellation was not acknowledged; resources retained (phase '
        ..tostring(state.phase)..', controller '..(absent and 'absent' or 'alive')..')',0)
    end
  end
end

function M.retire_completed(identity,options)
  local ref=M.reference(identity)
  local existing=M.status(identity,options)
  if not existing then return nil end
  if not R.terminal(existing.phase) then
    error('Recovery registration already exists',0)
  end
  os.remove(ref.state_file)
  os.remove(ref.control_file)
  return true
end

function M.step(identity,now,owner,launch,options)
  options=options or {}
  local ref=M.reference(identity)
  local read=options.read_state or M.read_state
  local write=options.write_state or M.write_state
  local state=read(ref.state_file,options)
  local control=read(ref.control_file,options)
  local event=options.event
  if event==nil and state then event=M.read_runner(state,options) end
  local pending
  local decision,changed,must_launch=R.decide{identity=identity,state=state,control=control,
    event=event,now=now,owner=owner,
    token=options.token or function() return M.new_token() end,
    runner_state=options.runner_state or function(generation)
      return identity.cwd:gsub('\\','/')..'/.uml-viewer/'..identity.session..'-runner-'..generation..'.edn'
    end}
  if not must_launch then
    if changed then write(ref.state_file,decision) end
    return decision
  end
  pending=decision
  -- Journal before the external side effect; a persisted :submitting state is
  -- never replayed after a controller rebind.
  write(ref.state_file,pending)
  local submission=R.copy(identity)
  submission.runner_token=pending.runner_token; submission.runner_state=pending.runner_state
  local ok,message=pcall(launch,submission)
  local result=R.copy(pending)
  if ok then
    result.phase='running'
  else
    result.phase='failed'; result.reason='Recovery submission uncertain: '..tostring(message)
  end
  write(ref.state_file,result)
  return result
end

local token_seed
function M.new_token()
  if token_seed==nil then
    local ok,pid=pcall(function() return Process.current_pid() end)
    pid=ok and pid or 0
    token_seed=(Clock.monotonic_ms()+os.time()*1000+pid)%2147483647
    math.randomseed(token_seed)
    math.random(); math.random()
  end
  local template='xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'
  return (template:gsub('[xy]',function(c)
    local random=math.random(0,15)
    if c=='y' then random=random%4+8 end
    return string.format('%x',random)
  end))
end

function M.start(identity,options)
  options=options or {}
  local ref=M.reference(identity)
  M.retire_completed(identity,options)
  M.write_state(ref.manifest,identity)
  if options.spawn then
    options.spawn(identity,ref.manifest)
  else
    local controller=assert(options.controller,'A detached controller launcher is required')
    local argv={}
    for _,value in ipairs(controller.argv or {}) do argv[#argv+1]=value end
    argv[#argv+1]=ref.manifest
    Process.spawn_detached(controller.executable,argv,identity.cwd,
      ref.state_file:gsub('recovery%.edn$','recovery.log'),
      ref.state_file:gsub('recovery%.edn$','recovery.log.err'))
  end
  local clock=options.clock or Clock.monotonic_ms
  local sleep=options.sleep or Clock.sleep
  local deadline=clock()+30000
  while true do
    if M.status(identity,options) then
      return M.verify(identity,M.default_owner,options)
    end
    if clock()<deadline then sleep(25) else
      error('Detached recovery controller did not register; ownership retained',0)
    end
  end
end

function M.run_child(identity,options)
  options=options or {}
  local argv=identity.command
  local lock=options.try_lock or function(path) return OS.try_acquire(path) end
  local generation_lock=lock(identity.runner_state..'.lock')
  if not generation_lock then error('Runner generation already executing',0) end
  local started_path=identity.runner_state..'.started.json'
  local exit_path=identity.runner_state..'.exit.json'
  os.remove(started_path); os.remove(exit_path)
  local start=options.start or Process.start_attached
  local waiter=start(argv,identity.cwd,{'NO_COLOR'},started_path,exit_path)
  local function read_json(path)
    local file=io.open(path,'rb'); if not file then return nil end
    local text=file:read('*a'); file:close()
    local ok,value=pcall(function() return require('uml.adapters.json').decode(text) end)
    return ok and value or nil
  end
  local started
  local clock=options.clock or Clock.monotonic_ms
  local sleep=options.sleep or Clock.sleep
  local deadline=clock()+30000
  repeat
    started=read_json(started_path)
    if not started and clock()>=deadline then break end
    if not started then sleep(25) end
  until started
  if not started then error('Runner child did not report its identity',0) end
  local child={pid=started.pid,start=started.start}
  local write=options.write_state or M.write_state
  write(identity.runner_state,{token=identity.runner_token,phase='started',child=child})
  local await=options.await_exit or function(value) return Process.await_exit(value,exit_path,options) end
  local exit=await(waiter)
  write(identity.runner_state,{token=identity.runner_token,phase='exited',child=child,exit=exit,at=clock()})
  OS.release(generation_lock)
  return exit
end

function M.controller_loop(identity,options)
  options=options or {}
  local clock=options.clock or Clock.monotonic_ms
  local sleep=options.sleep or Clock.sleep
  local owner=options.owner or M.default_owner
  local launch=options.launch or function() error('Recovery relaunch is not configured',0) end
  local lock=options.try_lock or function(path) return OS.try_acquire(path) end
  local handle=lock(M.reference(identity).lock_file)
  if not handle then error('Project recovery controller already running',0) end
  local current_pid=options.current_pid or Process.current_pid
  M.register(identity,owner(current_pid()),{now=clock()})
  while true do
    local ok,state=pcall(function()
      return M.with_gate(identity,function()
        return M.step(identity,clock(),owner,launch,options)
      end,options)
    end)
    if not ok then
      local existing=M.status(identity,options) or {}
      existing.phase='failed'; existing.reason=tostring(state)
      M.write_state(M.reference(identity).state_file,existing)
      state=existing
    end
    if R.terminal(state.phase) then
      if options.release~=false then OS.release(handle) end
      return state.phase
    end
    sleep(100)
  end
end
return M
