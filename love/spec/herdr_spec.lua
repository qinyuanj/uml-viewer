local JSON=require('uml.adapters.json')
local Herdr=require('uml.adapters.herdr')
local H=require('uml.domain.herdr')
local lfs=require('lfs')

local alphabet='ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/'
local function decode_base64(text)
  local out={}
  local buffer,count=0,0
  for i=1,#text do
    local char=text:sub(i,i)
    if char=='=' then break end
    local value=alphabet:find(char,1,true)
    if value then
      buffer=buffer*64+(value-1); count=count+6
      if count>=8 then
        count=count-8
        out[#out+1]=string.char(math.floor(buffer/2^count)%256)
        buffer=buffer%2^count
      end
    end
  end
  return table.concat(out)
end
local function decode_command(encoded)
  local bytes=decode_base64(encoded)
  local out={}
  for i=1,#bytes,2 do out[#out+1]=bytes:sub(i,i) end
  return table.concat(out)
end

local owned={backend='herdr',owner_role='companion',session='uml-viewer-A',
  socket='C:/herdr/sessions/A/herdr.sock',workspace='w1',pane='w1:p1',
  terminal_id='term_A',shell_pid=42,owner_start='shell-start',
  server_pid='41',server_start='server-start',cwd='C:/project',command={'claude.exe'}}
local function response(value) return {exit=0,out=JSON.encode(value),err=''} end
local function fake_cli(calls,state)
  return function(argv)
    calls[#calls+1]=argv
    local exe,selector,session=argv[1],argv[2],argv[3]
    if not (exe=='herdr' and selector=='--session' and session=='uml-viewer-A') then
      error('Unexpected selector: '..table.concat(argv,' '),0)
    end
    local op={argv[4],argv[5]}
    local fail=state.fail
    if fail and fail[1]==op[1] and fail[2]==op[2] then return {exit=1,out='',err='injected failure'} end
    if op[1]=='status' and op[2]=='server' then
      return response{session=session,socket=owned.socket,running=not state.stopped,
        version='0.9.3',protocol=22,compatible=true,endpoint_compatible=true}
    elseif op[1]=='api' and op[2]=='snapshot' then
      local panes=JSON.array{{pane_id='w1:p1',workspace_id='w1',terminal_id=state.terminal or 'term_A',cwd=state.cwd or owned.cwd}}
      local workspaces=state.closed and JSON.array{} or JSON.array{{workspace_id='w1'}}
      local agents=state.no_agent and JSON.array{} or JSON.array{{pane_id='w1:p1',terminal_id='term_A',agent='claude'}}
      return response{result={snapshot={version='0.9.3',protocol=22,workspaces=workspaces,panes=panes,agents=agents}}}
    elseif op[1]=='pane' and op[2]=='process-info' then
      local foreground
      if state.foreground=='absent' then foreground=nil
      elseif state.foreground then foreground=state.foreground
      else foreground=JSON.array{{pid=42,name='pwsh'}} end
      return response{result={process_info={pane_id='w1:p1',shell_pid=42,
        foreground_processes=foreground}}}
    elseif op[1]=='workspace' and op[2]=='create' then
      return response{result={workspace={workspace_id='w1'},root_pane={pane_id='w1:p1',terminal_id='term_A'}}}
    elseif op[1]=='workspace' and op[2]=='close' then
      if not state.ignore_close then state.closed=true end
      return response{result={type='ok'}}
    elseif op[1]=='session' and op[2]=='stop' then
      state.stopped=true
      return response{stopped=true,session={name=session,socket_path=owned.socket}}
    elseif op[1]=='session' and op[2]=='delete' then
      state.deleted=true
      return response{deleted=true,session={name=session,socket_path=owned.socket}}
    elseif op[1]=='session' and op[2]=='list' then
      if state.empty_sessions then return response{sessions=JSON.array{}} end
      return response{sessions=state.deleted and JSON.array{{name='uml-viewer-B'}}
        or JSON.array{{name=session},{name='uml-viewer-B'}}}
    elseif op[1]=='agent' and op[2]=='prompt' then
      state.prompted=true
      return response{result={type='ok'}}
    elseif op[1]=='pane' and op[2]=='send-keys' then
      return response{result={type='ok'}}
    elseif op[1]=='pane' and op[2]=='run' then
      state.run_text=argv[7]
      return response{result={type='ok'}}
    end
    error('Unexpected CLI call: '..table.concat(argv,' '),0)
  end
end
local function fake_owner(state)
  return function(pid)
    if tostring(pid)=='41' then
      if state.stopped then return nil end
      return {pid=41,start=state.server_start or 'server-start',name='herdr'}
    elseif tostring(pid)=='42' then
      return {pid=42,start=state.shell_start or 'shell-start',name='pwsh'}
    end
  end
end
local function backend(run,owner,extra)
  local identity=owned
  local overrides={}
  for key,value in pairs(extra or {}) do overrides[key]=value end
  if overrides.identity then identity=overrides.identity; overrides.identity=nil end
  local options={run=run,owner=owner,selected={path='herdr',release='0.9.3'},
    canonical=function(path) return (path or ''):gsub('\\','/') end,
    clock=function() return 0 end,sleep=function() end,
    read_companion=function() return nil end,
    write_companion=function(_,record) return record end}
  for key,value in pairs(overrides) do options[key]=value end
  return Herdr.new(identity,options)
end
local function error_of(fn)
  local ok,err=pcall(fn); assert.is_false(ok,'expected an error'); return err
end
local function ops(calls)
  local out={}
  for _,argv in ipairs(calls) do
    if argv[1]=='herdr' then out[#out+1]=(argv[4] or '')..' '..(argv[5] or '') end
  end
  return out
end

describe('Herdr owned session backend',function()
  it('binds the full identity via explicit session selectors',function()
    local calls={}; local state={}
    local B=backend(fake_cli(calls,state),fake_owner(state))
    assert.are.equal('w1:p1',B.bind('uml-viewer-A','w1:p1'))
    assert.is_true(B.probe('uml-viewer-A'))
  end)
  it('rejects A and B selectors even though both sessions use w1:p1',function()
    local calls={}; local state={}
    local B=backend(fake_cli(calls,state),fake_owner(state))
    assert.is_truthy(error_of(function() B.bind('uml-viewer-B','w1:p1') end))
    assert.are.equal(0,#calls)
  end)
  it('refuses cleanup when terminal, shell creation or server creation identity changes',function()
    for _,change in ipairs{{terminal='term_B'},{shell_start='reused'},{server_start='reused'}} do
      local calls={}; local state={}
      for key,value in pairs(change) do state[key]=value end
      local B=backend(fake_cli(calls,state),fake_owner(state))
      assert.is_truthy(error_of(function() B.cleanup('uml-viewer-A') end))
      local seen=table.concat(ops(calls),',')
      assert.is_nil(seen:find('workspace close',1,true))
      assert.is_nil(seen:find('session stop',1,true))
    end
  end)
  it('closes and confirms only A while leaving B in the inventory',function()
    local calls={}; local state={}
    local B=backend(fake_cli(calls,state),fake_owner(state))
    assert.are.same({released=true},B.cleanup('uml-viewer-A'))
    assert.is_true(state.deleted)
    for _,argv in ipairs(calls) do if argv[1]=='herdr' then assert.are.equal('uml-viewer-A',argv[3]) end end
  end)
  it('retains ambiguous cleanup after failure or a false successful close',function()
    for _,change in ipairs{{fail={'workspace','close'}},{ignore_close=true},{fail={'session','stop'}}} do
      local calls={}; local state={}
      for key,value in pairs(change) do state[key]=value end
      local B=backend(fake_cli(calls,state),fake_owner(state))
      assert.is_truthy(error_of(function() B.cleanup('uml-viewer-A') end))
      assert.is_nil(state.deleted)
    end
  end)
  it('never treats malformed or timed out probes as absence',function()
    for _,result in ipairs{{exit=0,out='not JSON'},{exit=nil,out='',timeout=true,err='timeout'}} do
      local B=backend(function() return result end,fake_owner({}))
      assert.is_truthy(error_of(function() B.probe('uml-viewer-A') end))
    end
  end)
  it('requires fresh consumption acknowledgement and a real Claude occupant',function()
    for _,initial in ipairs{{no_agent=true}, {}} do
      local calls={}; local state={}
      for key,value in pairs(initial) do state[key]=value end
      local B=backend(fake_cli(calls,state),fake_owner(state))
      assert.is_truthy(error_of(function()
        B.wake('w1:p1',{'-l','mail token-1'},{token='token-1',acknowledged=function() return false end,timeout_ms=0})
      end))
      assert.are.equal(not initial.no_agent,state.prompted==true)
    end
  end)
  it('returns success only after this prompt was consumed',function()
    local calls={}; local state={}
    local B=backend(fake_cli(calls,state),fake_owner(state))
    assert.are.same({status='acknowledged',token='token-2'},
      B.wake('w1:p1',{'-l','mail token-2'},{token='token-2',acknowledged=function() return state.prompted==true end}))
  end)
  it('rejects preexisting acknowledgements and missing callbacks before delivery',function()
    local calls={}; local state={}
    local B=backend(fake_cli(calls,state),fake_owner(state))
    assert.is_truthy(error_of(function() B.wake('w1:p1',{'-l','mail token'},{}) end))
    assert.is_truthy(error_of(function()
      B.wake('w1:p1',{'-l','mail token'},{token='token',acknowledged=function() return true end})
    end))
    assert.is_nil(state.prompted)
  end)
  it('persists observed provisional identity before run and returns original argv',function()
    local calls={}; local state={empty_sessions=true}; local checkpoints={}
    local run=fake_cli(calls,state)
    local function routed(argv)
      if argv[1]=='pwsh.exe' then return {exit=0,out='',err=''} end
      return run(argv)
    end
    local original={'claude.exe','space $ & \' 中文',''}
    local B=backend(routed,fake_owner(state),{
      identity={provisional=true},
      write_companion=function(_,record) checkpoints[#checkpoints+1]=record; return record end,
      spawn_server=function() return {pid=41,start='server-start',name='herdr'} end,
      runner={executable='love',argv={'love','C:/love','companion-runner'}},
      runner_state=function(record) return {token=record.runner_token,phase='started'} end,
      start_recovery=function(record) state.recovery=record end})
    local root=os.tmpname(); os.remove(root); assert(lfs.mkdir(root)); state.cwd=root
    local ok,identity=pcall(B.create,'uml-viewer-A',root,original)
    os.remove(root..'/.uml-viewer/uml-viewer-A-runner.edn.launch.edn')
    lfs.rmdir(root..'/.uml-viewer'); lfs.rmdir(root)
    assert.is_true(ok,identity)
    assert.are.same(original,identity.command)
    assert.is_nil(identity.provisional)
    assert.are.equal('term_A',identity.terminal_id)
    assert.are.equal('string',type(identity.runner_token))
    local before_run
    for _,record in ipairs(checkpoints) do
      if record.provisional then before_run=record end
      if record.pane and record.shell_pid then
        assert.are.equal(42,record.shell_pid)
        assert.are.equal('shell-start',record.owner_start)
      end
    end
    assert.is_not_nil(before_run)
    assert.is_not_nil(state.run_text)
    assert.is_not_nil(state.recovery)
    assert.is_nil(state.recovery.provisional)
  end)
  it('resumes cleanup after workspace closure only under the same server identity',function()
    local calls={}; local state={fail={'session','stop'}}; local checkpoint
    local B=backend(fake_cli(calls,state),fake_owner(state),{
      read_companion=function() return checkpoint end,
      write_companion=function(_,record) checkpoint=record; return record end})
    assert.is_truthy(error_of(function() B.cleanup('uml-viewer-A') end))
    assert.are.equal('server-stopping',checkpoint.cleanup_phase)
    state.fail=nil
    assert.are.same({released=true},B.cleanup('uml-viewer-A'))
    local closes=0
    for _,op in ipairs(ops(calls)) do if op=='workspace close' then closes=closes+1 end end
    assert.are.equal(1,closes)
  end)
  it('cancels recovery before destroying any resource and retains everything on failure',function()
    local record={}; for key,value in pairs(owned) do record[key]=value end
    record.supervisor={token='controller'}
    local calls={}; local state={}
    local order={}
    local B=Herdr.new(record,{run=function(argv)
      order[#order+1]=(argv[4] or '')..' '..(argv[5] or '')
      return fake_cli(calls,state)(argv)
    end,owner=fake_owner(state),selected={path='herdr',release='0.9.3'},
      canonical=function(p) return p end,clock=function() return 0 end,sleep=function() end,
      read_companion=function() return nil end,write_companion=function(_,r) return r end,
      cancel_recovery=function() order[#order+1]='cancel' end})
    assert.are.same({released=true},B.cleanup('uml-viewer-A'))
    assert.are.equal('cancel',order[1])
    local failed=Herdr.new(record,{run=function() error('must not execute',0) end,
      owner=fake_owner({}),selected={path='herdr',release='0.9.3'},
      canonical=function(p) return p end,clock=function() return 0 end,sleep=function() end,
      read_companion=function() return nil end,write_companion=function(_,r) return r end,
      cancel_recovery=function() error('cannot confirm release',0) end})
    assert.is_truthy(error_of(function() failed.cleanup('uml-viewer-A') end))
  end)
  it('ignores stale or malformed runner state and accepts only the current launch token',function()
    local name=os.tmpname(); os.remove(name)
    local file=assert(io.open(name,'wb')); file:write(JSON.encode{token='stale',phase='started'}); file:close()
    local B=backend(fake_cli({},{}),fake_owner({}))
    assert.is_nil(B.runner_state{runner_state=name,runner_token='current'})
    file=assert(io.open(name,'wb')); file:write('broken JSON'); file:close()
    assert.is_nil(B.runner_state{runner_state=name,runner_token='current'})
    file=assert(io.open(name,'wb')); file:write(JSON.encode{token='current',phase='exited',exit=37}); file:close()
    assert.are.equal(37,B.runner_state{runner_state=name,runner_token='current'}.exit)
    os.remove(name)
  end)
  it('explicitly refuses automatic recovery without a durable supervisor',function()
    local calls={}; local state={}
    local B=backend(fake_cli(calls,state),fake_owner(state))
    assert.are.same({enabled=false},B.recovery('uml-viewer-A',nil,nil,nil,false))
    assert.is_truthy(error_of(function() B.recovery('uml-viewer-A',nil,nil,nil,true) end))
  end)
end)

describe('Herdr recovery relaunch',function()
  local relaunch_identity={}
  for key,value in pairs(owned) do relaunch_identity[key]=value end
  relaunch_identity.runner_token='gen-token'
  -- Relaunch writes <runner_state>.launch.edn for real. A unique temporary
  -- name keeps concurrent mutation workers from replacing one shared file.
  relaunch_identity.runner_state=os.tmpname(); os.remove(relaunch_identity.runner_state)
  teardown(function() os.remove(relaunch_identity.runner_state..'.launch.edn') end)

  local function launch(calls,state,extra)
    local run=fake_cli(calls,state)
    local routed=function(argv)
      if argv[1]=='pwsh.exe' then
        calls[#calls+1]=argv
        if #argv>3 then state.reset_script=decode_command(argv[4]) end
        return {exit=(state.reset_exit or 0),out='',err=''}
      end
      return run(argv)
    end
    local options={run=routed,owner=fake_owner(state),selected={path='herdr',release='0.9.3'},
      canonical=function(p) return p end,clock=function() return 0 end,sleep=function() end,
      runner={executable='love',argv={'love','C:/love','companion-runner'}},
      read_companion=function() return nil end,write_companion=function(_,r) return r end}
    for key,value in pairs(extra or {}) do options[key]=value end
    local B=Herdr.new(relaunch_identity,options)
    B.relaunch(relaunch_identity)
    return B
  end
  local function op_of(argv) return (argv[4] or '')..' '..(argv[5] or '') end

  it('resets killed-TUI terminal modes and the input line before submitting',function()
    local calls={}; local state={}
    launch(calls,state)
    local reset_index,keys_index,run_index
    for index,argv in ipairs(calls) do
      if argv[1]=='pwsh.exe' then reset_index=index
      elseif op_of(argv)=='pane send-keys' then keys_index=index
      elseif op_of(argv)=='pane run' then run_index=index end
    end
    assert.is_not_nil(reset_index); assert.is_not_nil(keys_index); assert.is_not_nil(run_index)
    assert.is_true(reset_index<keys_index and keys_index<run_index)
    local script=state.reset_script
    assert.is_not_nil(script)
    assert.is_truthy(script:find('AttachConsole(42)',1,true))
    assert.is_truthy(script:find('CreateFile("CONOUT$"',1,true))
    assert.is_truthy(script:find('[?1049l',1,true))
    assert.is_truthy(script:find('[?2004l',1,true))
    assert.is_truthy(script:find('[<u',1,true))
    local text=state.run_text
    assert.is_true(text:sub(1,2)=='& ')
    assert.is_truthy(text:find("'companion-runner'",1,true))
  end)
  it('fails closed without submitting when the terminal reset fails',function()
    local calls={}; local state={reset_exit=3}
    assert.is_truthy(error_of(function() launch(calls,state) end))
    for _,argv in ipairs(calls) do
      assert.are_not.equal('pane run',op_of(argv))
      assert.are_not.equal('pane send-keys',op_of(argv))
    end
  end)
  it('refuses submission when the pane foreground is unknown or occupied',function()
    for _,foreground in ipairs{'absent', JSON.array{{pid=42,name='pwsh'},{pid=7,name='claude'}}} do
      local calls={}; local state={foreground=foreground}
      assert.is_truthy(error_of(function() launch(calls,state) end))
      for _,argv in ipairs(calls) do
        assert.are_not.equal('pwsh.exe',argv[1])
        assert.are_not.equal('pane run',op_of(argv))
      end
    end
  end)
  it('retries only canonical-mode pane run rejections within the deadline',function()
    local calls={}; local state={}
    local run=fake_cli(calls,state)
    local attempts=0
    local B=Herdr.new(relaunch_identity,{
      run=function(argv)
        if argv[1]=='pwsh.exe' then return {exit=0,out='',err=''} end
        if op_of(argv)=='pane run' then
          attempts=attempts+1
          if attempts<3 then return {exit=1,out='',err='pane_send_failed: input exceeds the canonical mode limit'} end
          return {exit=0,out='{}',err=''}
        end
        return run(argv)
      end,
      owner=fake_owner(state),selected={path='herdr',release='0.9.3'},
      canonical=function(p) return p end,clock=function() return 0 end,sleep=function() end,
      runner={executable='love',argv={'runner'}},
      read_companion=function() return nil end,write_companion=function(_,r) return r end})
    B.relaunch(relaunch_identity)
    assert.are.equal(3,attempts)
  end)
end)

describe('Herdr pane command shell detection',function()
  it('accepts macOS login shells whose argv0 carries a leading dash',function()
    for _,shell in ipairs{'-zsh','-bash','-sh','/bin/-zsh'} do
      assert.are.equal("'echo' 'hi there'",H.pane_command({'echo','hi there'},shell))
    end
  end)
  it('still rejects unsupported shells',function()
    assert.is_truthy(error_of(function() H.pane_command({'echo','hi'},'fish') end))
  end)
end)

describe('Herdr pane run delivery',function()
  it('does not retry other pane run failures',function()
    local attempts,sleeps=0,0
    local B=backend(function()
      attempts=attempts+1
      return {exit=1,out='',err='pane_not_found'}
    end,function() return nil end,{clock=function() return 0 end,sleep=function() sleeps=sleeps+1 end})
    assert.is_truthy(error_of(function() B.pane_run('uml-viewer-A','w1:p1','text') end))
    assert.are.equal(1,attempts)
    assert.are.equal(0,sleeps)
  end)
  it('stops retrying once the delivery deadline is exhausted',function()
    local ticks={0,5000,20000}
    local index,attempts,sleeps=0,0,0
    local B=backend(function()
      attempts=attempts+1
      return {exit=1,out='',err='pane_send_failed: input exceeds the canonical mode limit'}
    end,function() return nil end,{
      clock=function() index=index+1; return ticks[index] end,
      sleep=function() sleeps=sleeps+1 end})
    assert.is_truthy(error_of(function() B.pane_run('uml-viewer-A','w1:p1','text') end))
    assert.are.equal(2,attempts)
    assert.are.equal(1,sleeps)
    assert.are.equal(3,index)
  end)
end)
