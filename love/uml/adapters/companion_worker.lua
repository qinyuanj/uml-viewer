-- One isolated LOVE process owns lifecycle side effects. The UI exchanges
-- generation-scoped files and never waits for subprocess stdout or cleanup.
local P=require('uml.adapters.process')
local R=require('uml.adapters.recovery')
local L=require('uml.adapters.companion_lifecycle')
local E=require('uml.edn')
local Clock=require('uml.adapters.clock')
local Lock=require('uml.adapters.lock')
local M={}
local function write(path,value)
  local temporary=path..'.tmp'
  local f=assert(io.open(temporary,'wb')); assert(f:write(E.encode(R.to_edn(value)))); f:close()
  assert(os.rename(temporary,path)) -- destination is unique, never replaced
end
local function read(path)
  local f=io.open(path,'rb'); if not f then return nil end
  local text=f:read('*a'); f:close()
  local ok,value=pcall(E.parse,text)
  if not ok then return nil end
  return R.from_edn(value)
end
function M.start(root,restart)
  local token=R.new_token()
  local base=root..'/.uml-viewer/companion-worker-'..token
  local request={token=token,root=root,restart=restart,base=base,viewer=P.owner_native(P.current_pid())}
  write(base..'.request.edn',request)
  local owner=P.spawn_native(love.filesystem.getExecutablePath(),
    {love.filesystem.getSource(),'companion-worker',base..'.request.edn'},root)
  return {base=base,token=token,owner=owner,sequence=0,seen=0,phase='starting',started=Clock.monotonic_ms()}
end
function M.command(worker,operation,extra)
  worker.sequence=worker.sequence+1
  write(worker.base..'.command-'..worker.sequence..'.edn',
    {token=worker.token,operation=operation,extra=extra})
end
function M.poll(worker,services)
  services=services or {}
  local read=services.read or read
  local P=services.process or P
  local Clock=services.clock or Clock
  local command=services.command or M.command
  local index=worker.seen+1
  for _=1,32 do
    local event=read(worker.base..'.event-'..index..'.edn')
    if not event then break end
    if event.token~=worker.token then error('Companion helper result identity mismatch; ownership retained',0) end
    worker.seen=index
    if not worker.timed_out or event.phase=='stopped' or event.phase=='failed' then
      worker.phase=event.phase; worker.message=event.message
      if event.phase=='ready' then worker.started=nil end
    end
    index=index+1
  end
  if worker.started and not worker.timed_out and worker.phase~='ready'
      and worker.phase~='failed' and worker.phase~='stopped'
      and Clock.monotonic_ms()-worker.started>120000 then
    worker.timed_out=true; worker.phase='failed'
    worker.message='Companion startup timed out; cancellation pending, ownership retained'
    command(worker,'stop',{keep=false})
  end
  local observed=P.owner_native(worker.owner.pid)
  if not observed or observed.start~=worker.owner.start then
    if worker.phase~='stopped' and worker.phase~='failed' then
      worker.phase='failed'; worker.message='Companion helper exited without a final result; ownership retained'
    end
    worker.exited=true
  end
  return worker
end
function M.run(request_path,services)
  services=services or {}
  local read=services.read or read
  local write=services.write or write
  local P=services.process or P
  local L=services.lifecycle or L
  local Clock=services.clock or Clock
  local Lock=services.lock or Lock
  local request=assert(read(request_path),'Missing companion helper request')
  local sequence=0
  local function event(phase,message)
    sequence=sequence+1
    write(request.base..'.event-'..sequence..'.edn',{token=request.token,phase=phase,message=message})
  end
  local lock=Lock.try_acquire(request.root..'/.uml-viewer/companion-worker.lock')
  if not lock then event('failed','Another companion helper owns this project; ownership retained'); return 1 end
  local next_command=1
  local pending={}
  local stop,keep=false,false
  local function commands()
    while true do
      local command=read(request.base..'.command-'..next_command..'.edn')
      if not command then break end
      assert(command.token==request.token,'Helper command identity mismatch')
      next_command=next_command+1
      if command.operation=='stop' then stop=true; keep=command.extra and command.extra.keep==true
      elseif command.operation=='wake' then pending[#pending+1]=command.extra.id end
    end
    local owner=P.owner_native(request.viewer.pid)
    if not owner or owner.start~=request.viewer.start then stop=true end
  end
  local record,B
  local options={canonical=services.canonical or require('uml.adapters.platform_path').canonical,
    progress=function(stage)
      event(stage); commands()
      if stop then error('Companion startup cancelled',0) end
    end}
  local ok,message=pcall(function()
    event('starting')
    record,B=(request.restart and L.restart or L.create)(request.root,options)
    commands()
    if not stop then
      local failure
      if not request.restart and services.open_window then
        local opened,failed=pcall(services.open_window,record.session)
        if not opened then
          failure='Terminal attach failed: '..tostring(failed)
        else
          -- Nothing attaches automatically on the supported platforms; the
          -- ready event carries the command a person can run in a terminal.
          failure='Companion started; attach manually: '
            ..tostring(type(failed)=='table' and failed.attach or 'herdr session attach '..tostring(record.session))
        end
      end
      event('ready',failure)
    end
    local G=require('uml.domain.geom')
    local mailbox=require('uml.adapters.mailbox').open(request.root..'/.uml-viewer/to-agent.edn')
    while not stop do
      commands()
      if not stop and #pending>0 then
        local id=table.remove(pending,1)
        local delivered,reason=pcall(B.wake,record.pane,{'-l','Mailbox mail '..tostring(id)},{
          token='mail-'..tostring(id),timeout_ms=5000,
          acknowledged=function()
            for _,queued in ipairs(G.get(mailbox.read(),'queue') or {}) do
              if G.get(queued,'id')==id then return false end
            end
            return true
          end})
        if not delivered then event('ready','Companion wake failed: '..tostring(reason)) end
      end
      if not stop then Clock.sleep(50) end
    end
    event('stopping')
    if not keep then L.shutdown(request.root,options) end
    event('stopped')
  end)
  if not ok then
    -- A completed record may be released only through the existing verified
    -- cleanup contract. Partial/unknown records remain for diagnosis.
    if stop and not keep then
      local loaded,owned=pcall(L.required,request.root,options)
      if loaded then
        local released,reason=pcall(L.shutdown,request.root,{canonical=options.canonical})
        if released then event('stopped','Startup cancelled; owned resources released')
        else event('failed','Startup cancelled: '..tostring(reason)..'; ownership retained') end
      else event('failed',tostring(message)..'; incomplete ownership retained') end
    else event('failed',tostring(message)..'; ownership retained') end
  end
  Lock.release(lock)
  return ok and 0 or 1
end
return M
