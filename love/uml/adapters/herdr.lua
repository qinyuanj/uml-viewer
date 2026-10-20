-- Herdr session backend. Every CLI request selects the recorded session; the
-- selected executable is fixed for this process. Ownership is EDN on disk and
-- plain tables in memory.
local JSON=require('uml.adapters.json')
local Ownership=require('uml.adapters.ownership')
local Process=require('uml.adapters.process')
local Clock=require('uml.adapters.clock')
local Recovery=require('uml.adapters.recovery')
local Text=require('uml.domain.text')
local H=require('uml.domain.herdr')
local C=require('uml.domain.companion')
local M={}

local function clean(value)
  if value==JSON.null then return nil end
  if type(value)=='table' then
    for key,item in pairs(value) do value[key]=clean(item) end
  end
  return value
end

function M.new(identity,options)
  options=options or {}
  local B={identity=identity,options=options}
  B.run=options.run or Process.run
  B.owner=options.owner or Process.owner
  B.clock=options.clock or Clock.monotonic_ms
  B.sleep=options.sleep or Clock.sleep
  B.canonical=options.canonical or function(path) return (path or ''):gsub('\\','/') end
  B.selected=options.selected
  B.write_companion=options.write_companion or function(root,record)
    return Ownership.write(root,Recovery.to_edn(record))
  end
  B.read_companion=options.read_companion or function(root)
    local value=Ownership.read(root); if value==nil then return nil end
    return Recovery.from_edn(value)
  end

  function B.herdr() return (B.selected and B.selected.path) or 'herdr' end

  function B.fail(message) error(message,0) end

  local function checked(argv)
    local result=B.run(argv)
    if result.timeout or result.exit~=0 then
      B.fail('Companion command failed: '..tostring(argv[1])..': '..tostring(result.err or ''))
    end
    return result
  end

  local function parse(result)
    local ok,value=pcall(JSON.decode,result.out or '')
    if not ok or type(value)~='table' then B.fail('Malformed Herdr JSON') end
    value=clean(value)
    if value.error then B.fail('Herdr API error') end
    return value
  end

  function B.cli(session,...)
    local extra={...}
    local argv={B.herdr(),'--session',session}
    for _,value in ipairs(extra) do argv[#argv+1]=value end
    return parse(checked(argv))
  end

  function B.session_check(session)
    if not H.session_valid(session) then
      B.fail('Herdr requires an isolated uml-viewer named session')
    end
  end

  function B.status(session)
    local value=B.cli(session,'status','server','--json')
    local socket=value.socket
    if value.session~=session or type(socket)~='string'
        or not (socket:match('^%a:[\\/]') or socket:sub(1,1)=='/' or socket:sub(1,1)=='\\')
        or type(value.running)~='boolean' then
      B.fail('Ambiguous Herdr endpoint')
    end
    return value
  end

  function B.compatible(status)
    if status.running~=true or not C.herdr_version_supported(status.version)
        or status.protocol~=22 or status.compatible~=true or status.endpoint_compatible~=true then
      B.fail('Herdr endpoint is not a verified >=0.9.3/protocol 22 server')
    end
  end

  function B.snapshot(session)
    local data=B.cli(session,'api','snapshot')
    local snapshot=data.result and data.result.snapshot
    if type(snapshot)~='table' or not C.herdr_version_supported(snapshot.version)
        or snapshot.protocol~=22 or type(snapshot.panes)~='table' or type(snapshot.workspaces)~='table' then
      B.fail('Invalid Herdr snapshot')
    end
    return snapshot
  end

  function B.process_owner(pid)
    local text=tostring(pid)
    if not text:match('^[1-9]%d*$') then B.fail('Invalid owned process PID') end
    local observed=B.owner(tonumber(text))
    return observed
  end

  function B.owner_matches(pid,start)
    local observed=B.process_owner(pid)
    if not observed or observed.start~=start then
      B.fail('Herdr process creation identity changed')
    end
    return observed
  end

  function B.poll(timeout,interval,fn,message)
    local deadline=B.clock()+timeout
    while true do
      local value=fn()
      if value then return value end
      if B.clock()<deadline then B.sleep(interval) else B.fail(message) end
    end
  end

  function B.launch_server(session,cwd)
    -- Server diagnostics belong beside the project's other lifecycle journals;
    -- without them a failed start is an unexplained timeout.
    local spawn=B.options.spawn_server or function(executable,argv,directory)
      local log=directory:gsub('\\','/')..'/.uml-viewer/herdr-server.log'
      return Process.spawn_detached(executable,argv,directory,log,log..'.err')
    end
    local owner=spawn(B.herdr(),{'--session',session,'server'},cwd)
    if not owner then B.fail('Detached Herdr server exited') end
    return owner
  end

  function B.observe(expected,session,pane)
    H.complete(expected)
    if expected.session~=session or expected.pane~=pane then
      B.fail('Herdr selector conflicts with ownership')
    end
    local status=B.status(session)
    B.compatible(status)
    if expected.socket~=status.socket then B.fail('Herdr socket changed') end
    B.owner_matches(expected.server_pid,expected.server_start)
    local snapshot=B.snapshot(session)
    local info=B.cli(session,'pane','process-info','--pane',pane)
    local process_info=info.result and info.result.process_info
    H.observe(expected,status,snapshot,process_info,B.canonical)
    B.owner_matches(expected.shell_pid,expected.owner_start)
    return expected
  end

  function B.runner_state(record)
    if B.options.runner_state then return B.options.runner_state(record) end
    local path=record.runner_state
    if not path then return nil end
    local file=io.open(path,'rb'); if not file then return nil end
    local text=file:read('*a'); file:close()
    local value
    local ok,parsed=pcall(function() return require('uml.edn').parse(text) end)
    if ok then
      value=Recovery.from_edn(parsed)
    else
      local decoded_ok,decoded=pcall(JSON.decode,text)
      if decoded_ok then value=clean(decoded) end
    end
    if type(value)~='table' then return nil end
    if value.token~=record.runner_token then return nil end
    if value.phase then value.phase=tostring(value.phase) end
    return value
  end

  -- The runner must exec the agent directly so its recorded identity is the
  -- agent's own process, never a shell that re-parses argv.
  function B.require_direct_command(command)
    local first=tostring(command[1] or '')
    if first=='' then B.fail('Herdr command must be a nonempty argv vector') end
    local resolved=first
    if not resolved:find('/',1,true) then
      for directory in (os.getenv('PATH') or ''):gmatch('[^:]+') do
        local candidate=directory..'/'..first
        local handle=io.open(candidate,'rb')
        if handle then handle:close(); resolved=candidate; break end
      end
    end
    local handle=resolved and io.open(resolved,'rb')
    if not handle then B.fail('Companion executable not found on PATH: '..first) end
    handle:close()
    return resolved
  end

  function B.runner_command(record,command)
    local first=B.require_direct_command(command)
    if first~=command[1] then command=H.copy(command); command[1]=first end
    local manifest=record.runner_state..'.launch.edn'
    local copy=H.copy(record); copy.command=H.copy(command)
    Recovery.write_state(manifest,copy)
    local runner=B.options.runner
    if not runner then B.fail('Herdr runner launcher is not configured') end
    local argv={}
    for _,value in ipairs(runner.argv or {}) do argv[#argv+1]=value end
    argv[#argv+1]=manifest
    return argv
  end

  function B.pane_run(session,pane,text)
    local argv={B.herdr(),'--session',session,'pane','run',pane,text}
    local deadline=B.clock()+10000
    while true do
      local result=B.run(argv)
      if result.exit==0 and not result.timeout then return end
      local output=tostring(result.out or '')..tostring(result.err or '')
      if B.clock()<deadline and output:find('canonical mode',1,true) then
        B.sleep(100)
      else
        B.fail('Herdr pane run failed')
      end
    end
  end

  -- Reset what a previous agent left enabled. Mouse tracking modes must go
  -- first: a pane still reporting SGR mouse events delivers them to the
  -- replacement agent as keystrokes, which renders as garbage on its prompt.
  local RESET_SEQUENCE='\27[?1003l\27[?1002l\27[?1006l\27[?1000l'
    ..'\27[?1049l\27[?2004l\27[<u\27[<u\27[<u\27[?25h'

  function B.terminal_reset(record)
    -- No console handle exists on POSIX; write the sequence to the pane's own
    -- terminal by running printf inside that pane.
    B.pane_run(record.session,record.pane,'printf %s '..Text.shell_word(RESET_SEQUENCE))
    B.sleep(250)
    checked({B.herdr(),'--session',record.session,'pane','send-keys',record.pane,'ctrl+c'})
    B.sleep(150)
  end

  function B.relaunch(record)
    if not B.selected and B.options.select then B.selected=B.options.select() end
    B.observe(record,record.session,record.pane)
    local info=B.cli(record.session,'pane','process-info','--pane',record.pane)
    local process_info=info.result and info.result.process_info
    H.foreground_allowed(process_info,record.shell_pid)
    B.terminal_reset(record)
    local shell=B.process_owner(record.shell_pid)
    local command=record.recovery_command or record.command
    B.pane_run(record.session,record.pane,H.pane_command(B.runner_command(record,command),shell.name))
  end

  local function cancel_recovery(record)
    local cancel=B.options.cancel_recovery or function(value) return Recovery.cancel(value,B.options.cancel) end
    return cancel(record)
  end

  function B.recovery_status(record)
    if record.supervisor then return Recovery.status(record) end
  end

  -- Herdr derives its API socket from XDG_CONFIG_HOME and the session name.
  -- Measured against 0.9.5: a 101-byte path works and 102 fails, and the
  -- failure surfaces only later as a closed-channel error on workspace create,
  -- so refuse up front with the actual cause.
  function B.socket_check(session)
    local root=os.getenv('XDG_CONFIG_HOME')
    if not root or root=='' then root=(os.getenv('HOME') or '')..'/.config' end
    local path=('%s/herdr/sessions/%s/herdr.sock'):format(root:gsub('/+$',''),session)
    local limit=B.options.max_socket_bytes or 101
    if #path>limit then
      B.fail('Herdr socket path exceeds the Unix socket limit ('..#path..' > '..limit..'): '
        ..path..'; set a shorter XDG_CONFIG_HOME or project path')
    end
  end

  function B.create(session,cwd,command)
    B.session_check(session)
    B.socket_check(session)
    if type(command)~='table' or #command==0 then B.fail('Herdr command must be a nonempty argv vector') end
    for _,value in ipairs(command) do if type(value)~='string' then B.fail('Herdr command must be a nonempty argv vector') end end
    local resolved=B.require_direct_command(command)
    if resolved~=command[1] then command=H.copy(command); command[1]=resolved end
    if B.identity and not B.identity.provisional then B.fail('Cannot create over existing Herdr ownership') end
    local sessions=B.cli(session,'session','list','--json').sessions
    if type(sessions)~='table' then B.fail('Ambiguous session inventory') end
    for _,entry in ipairs(sessions) do if entry.name==session then B.fail('Herdr named session already exists') end end
    local base=H.copy(B.identity or {})
    base.backend='herdr'; base.owner_role='companion'; base.session=session
    base.cwd=B.canonical(cwd); base.command=H.copy(command); base.provisional=true
    B.write_companion(base.cwd,base)
    if B.options.progress then B.options.progress('server') end
    local owner=B.launch_server(session,base.cwd)
    base.server_pid=tostring(owner.pid); base.server_start=owner.start
    B.write_companion(base.cwd,base)
    local status=B.poll(5000,50,function()
      local value=B.status(session); if value.running then return value end
    end,'Herdr server did not become reachable')
    B.compatible(status)
    base.socket=status.socket
    B.write_companion(base.cwd,base)
    if B.options.progress then B.options.progress('workspace') end
    local created=B.cli(session,'workspace','create','--cwd',base.cwd,'--no-focus').result
    local pane=created.root_pane
    base.workspace=created.workspace.workspace_id; base.pane=pane.pane_id; base.terminal_id=pane.terminal_id
    B.write_companion(base.cwd,base)
    local info=B.cli(session,'pane','process-info','--pane',base.pane).result.process_info
    local shell=B.process_owner(info.shell_pid)
    base.shell_pid=info.shell_pid; base.owner_start=shell.start
    base.runner_token=Recovery.new_token()
    local root=(base.cwd:gsub('\\','/'))..'/.uml-viewer/'
    base.runner_state=root..session..'-runner.edn'
    base.supervisor={token=Recovery.new_token(),manifest=root..'recovery-launch.edn',
      state_file=root..'recovery.edn',control_file=root..'recovery-control.edn',
      gate_file=root..'recovery-gate.lock',lock_file=root..'recovery-controller.lock'}
    B.write_companion(base.cwd,base)
    B.observe(base,session,base.pane)
    if B.options.progress then B.options.progress('runner') end
    B.pane_run(session,base.pane,H.pane_command(B.runner_command(base,command),shell.name))
    local state=B.poll(30000,100,function() return B.runner_state(base) end,
      'Herdr runner did not acknowledge startup')
    if not (state.phase=='started' or (state.phase=='exited' and state.exit==0)) then
      B.fail('Herdr runner failed during startup')
    end
    B.observe(base,session,base.pane)
    local owned=H.copy(base); owned.provisional=nil
    B.write_companion(owned.cwd,owned)
    if B.options.progress then B.options.progress('recovery') end
    local start=B.options.start_recovery or Recovery.start
    start(owned,B.options.recovery)
    if B.options.progress then B.options.progress('committed') end
    B.identity=owned
    return owned
  end

  function B.identity_observe(session,pane) return B.observe(B.identity,session,pane) end
  function B.probe(session) B.observe(B.identity,session,B.identity.pane); return true end
  function B.bind(session,pane) return B.observe(B.identity,session,pane).pane end

  function B.wake(pane,keys,opts)
    opts=opts or {}
    local token=opts.token; local acknowledged=opts.acknowledged
    local timeout=opts.timeout_ms or 5000; local interval=opts.interval_ms or 50
    H.wake_arguments(token,acknowledged,keys)
    B.observe(B.identity,B.identity.session,pane)
    if acknowledged(token) then B.fail('Herdr wake acknowledgement predates this delivery') end
    local snapshot=B.snapshot(B.identity.session)
    local occupied=false
    for _,agent in ipairs(snapshot.agents or {}) do
      if agent.pane_id==pane and agent.terminal_id==B.identity.terminal_id and agent.agent=='claude' then occupied=true end
    end
    if not occupied then B.fail('No Claude agent occupies the owned Herdr pane') end
    checked({B.herdr(),'--session',B.identity.session,'agent','prompt',pane,keys[2]})
    B.poll(timeout,interval,function() return acknowledged(token) end,
      'Herdr wake consumption was not acknowledged; mail remains queued')
    B.observe(B.identity,B.identity.session,pane)
    return {status='acknowledged',token=token}
  end

  function B.recovery(session,pane,cwd,command,enabled)
    B.observe(B.identity,session,pane or B.identity.pane)
    if B.identity.supervisor then
      if enabled then Recovery.verify(B.identity,B.options.owner or Recovery.default_owner) return {enabled=true}
      else return cancel_recovery(B.identity) end
    end
    if enabled then B.fail('No durable supervisor registered; refusing replacement') end
    return {enabled=false}
  end

  function B.cleanup(session)
    local record=B.identity
    H.complete(record)
    if session~=record.session then B.fail('Cleanup selector conflicts with ownership') end
    if record.supervisor then cancel_recovery(record) end
    local saved=B.read_companion(record.cwd)
    if saved and next(saved)~=nil and H.ownership_changed(record,saved) then
      B.fail('Cleanup ownership changed')
    end
    local function checkpoint(phase)
      local value=H.copy(record); value.cleanup_phase=phase
      B.write_companion(record.cwd,value)
    end
    local phase=saved and saved.cleanup_phase or record.cleanup_phase
    local endpoint=B.status(session)
    if record.socket~=endpoint.socket then B.fail('Cleanup socket changed') end
    if not phase then
      B.observe(record,session,record.pane)
      checkpoint('workspace-closing')
    end
    if phase==nil or phase=='workspace-closing' or phase=='workspace-closed' then
      B.compatible(endpoint)
      B.owner_matches(record.server_pid,record.server_start)
      local snapshot=B.snapshot(session)
      local present=false
      for _,workspace in ipairs(snapshot.workspaces) do
        if workspace.workspace_id==record.workspace then present=true end
      end
      if present then
        B.observe(record,session,record.pane)
        B.cli(session,'workspace','close',record.workspace)
      end
      present=false
      for _,workspace in ipairs(B.snapshot(session).workspaces) do
        if workspace.workspace_id==record.workspace then present=true end
      end
      if present then B.fail('Herdr workspace remains after close') end
      checkpoint('workspace-closed')
    end
    if not H.cleanup_phase_valid(phase) then B.fail('Unknown cleanup phase') end
    if phase~='server-stopped' and phase~='session-deleting' then
      checkpoint('server-stopping')
      endpoint=B.status(session)
      if record.socket~=endpoint.socket then B.fail('Cleanup socket changed') end
      if endpoint.running then
        B.compatible(endpoint)
        B.owner_matches(record.server_pid,record.server_start)
        local stopped=B.cli(session,'session','stop',session,'--json')
        if not (stopped.stopped==true and stopped.session and stopped.session.name==session
            and stopped.session.socket_path==record.socket) then
          B.fail('Herdr session stop was not confirmed')
        end
      elseif B.process_owner(record.server_pid) then
        B.fail('Stopped endpoint has a live or reused server PID')
      end
      B.poll(3000,50,function() return B.process_owner(record.server_pid)==nil end,
        'Herdr server process remains after stop')
      checkpoint('server-stopped')
    end
    endpoint=B.status(session)
    if not (endpoint.running==false and endpoint.socket==record.socket
        and B.process_owner(record.server_pid)==nil) then
      B.fail('Herdr endpoint remains after stop')
    end
    checkpoint('session-deleting')
    local sessions=B.cli(session,'session','list','--json').sessions
    if type(sessions)~='table' then B.fail('Ambiguous cleanup inventory') end
    for _,entry in ipairs(sessions) do
      if entry.name==session then
        local deleted=B.cli(session,'session','delete',session,'--json')
        if not (deleted.deleted==true and deleted.session and deleted.session.name==session
            and deleted.session.socket_path==record.socket) then
          B.fail('Herdr session deletion was not confirmed')
        end
        break
      end
    end
    sessions=B.cli(session,'session','list','--json').sessions
    if type(sessions)~='table' then B.fail('Ambiguous cleanup inventory') end
    for _,entry in ipairs(sessions) do
      if entry.name==session then B.fail('Herdr session remains after deletion') end
    end
    return {released=true}
  end

  return B
end

return M
