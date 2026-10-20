local W=require('uml.adapters.companion_worker')
describe('isolated companion lifecycle',function()
  local function fixture(restart,cancel,fail,boundary,complete,locked,open_window)
    -- A missing parent makes any accidental fallback to the real journal writer
    -- fail instead of creating files in the mutation worker.
    local files={request={token='generation',root='project',base='missing-directory/journal',restart=restart,viewer={pid=1,start='original'}}}
    local calls,events={},{}
    local function stop() files['missing-directory/journal.command-1.edn']={token='generation',operation='stop',extra={keep=restart}} end
    local lifecycle={}
    local function start(root,options)
      calls[#calls+1]=restart and 'restart' or 'create'
      for _,stage in ipairs{'server','workspace','runner','recovery'} do
        if boundary==stage then stop() end
        options.progress(stage)
      end
      if fail then error('startup failure') end
      if cancel then stop() end
      return {session='owned'},{}
    end
    lifecycle.create=start; lifecycle.restart=start
    lifecycle.shutdown=function() calls[#calls+1]='shutdown' end
    lifecycle.required=function()
      if not complete then error('Incomplete ownership') end
      return {session='owned'}
    end
    local services={
      canonical=function(path) return path end,
      read=function(path) return files[path] end,
      write=function(path,event) events[#events+1]=event; files[path]=event end,
      process={owner_native=function() return {pid=1,start='original'} end},
      lifecycle=lifecycle,
      open_window=open_window,
      clock={sleep=function() stop() end},
      lock={try_acquire=function() return not locked end,release=function() calls[#calls+1]='release' end}}
    return W.run('request',services),calls,events
  end
  it('asks for the companion session on a fresh launch',function()
    local opened
    local code,_,events=fixture(false,false,false,nil,nil,nil,function(session)
      opened=session; return {terminal='none',session=session,attach='herdr session attach '..session}
    end)
    assert.equal(0,code); assert.equal('owned',opened)
    local ready
    for _,event in ipairs(events) do if event.phase=='ready' then ready=event end end
    -- Nothing attaches automatically on the supported platforms, so ready must
    -- always carry the command a person can run themselves.
    assert.equal('Companion started; attach manually: herdr session attach owned',ready.message)
  end)
  it('reports attach failure without losing the companion',function()
    local _,_,events=fixture(false,false,false,nil,nil,nil,function()
      error('no terminal available')
    end)
    local ready
    for _,event in ipairs(events) do if event.phase=='ready' then ready=event end end
    assert.is_truthy(ready.message:find('Terminal attach failed: ',1,true))
    assert.is_truthy(ready.message:find('no terminal available',1,true))
  end)
  it('does not open a second terminal when restarting',function()
    local calls=0
    fixture(true,false,false,nil,nil,nil,function() calls=calls+1 end)
    assert.equal(0,calls)
  end)
  it('starts and shuts down only after asynchronous stop',function()
    local code,calls,events=fixture(false,false,false)
    assert.equal(0,code); assert.same({'create','shutdown','release'},calls)
    assert.equal('stopped',events[#events].phase)
  end)
  it('retains the companion on restart stop',function()
    local code,calls=fixture(true,false,false)
    assert.equal(0,code); assert.same({'restart','release'},calls)
  end)
  it('never publishes ready after cancellation during startup',function()
    local code,calls,events=fixture(false,true,false)
    assert.equal(0,code); assert.same({'create','shutdown','release'},calls)
    for _,event in ipairs(events) do assert.is_not.equal('ready',event.phase) end
  end)
  it('retains failed or incomplete ownership without replacement',function()
    local code,calls,events=fixture(false,false,true)
    assert.equal(1,code); assert.same({'create','release'},calls)
    assert.equal('failed',events[#events].phase)
    assert.matches('ownership retained',events[#events].message)
  end)
  it('cancels at a startup boundary and cleans only verified complete ownership',function()
    local code,calls,events=fixture(false,false,false,'workspace',true)
    assert.equal(1,code); assert.same({'create','shutdown','release'},calls)
    assert.equal('stopped',events[#events].phase)
    for _,event in ipairs(events) do assert.is_not.equal('ready',event.phase) end
  end)
  it('retains incomplete ownership when cancelled at a startup boundary',function()
    local code,calls,events=fixture(false,false,false,'workspace',false)
    assert.equal(1,code); assert.same({'create','release'},calls)
    assert.equal('failed',events[#events].phase)
    assert.matches('incomplete ownership retained',events[#events].message)
  end)
  it('refuses a second helper without creating or cleaning resources',function()
    local code,calls,events=fixture(false,false,false,nil,false,true)
    assert.equal(1,code); assert.same({},calls)
    assert.equal('failed',events[#events].phase)
    assert.matches('Another companion helper',events[#events].message)
  end)
  local function poll_fixture(events,observed)
    local worker={base='journal',token='generation',owner={pid=2,start='original'},seen=0,phase='starting'}
    local services={
      read=function(path) local index=tonumber(path:match('event%-(%d+)')); return events[index] end,
      process={owner_native=function(pid) assert.equal(2,pid); return observed end}}
    return worker,services
  end
  it('consumes matching startup events in order',function()
    local worker,services=poll_fixture({{token='generation',phase='server'},{token='generation',phase='ready'}},{pid=2,start='original'})
    W.poll(worker,services)
    assert.equal(2,worker.seen); assert.equal('ready',worker.phase)
    assert.is_nil(worker.exited)
  end)
  it('rejects stale generation results before advancing the cursor',function()
    local worker,services=poll_fixture({{token='stale',phase='ready'}},{pid=2,start='original'})
    assert.has_error(function() W.poll(worker,services) end,'Companion helper result identity mismatch; ownership retained')
    assert.equal(0,worker.seen); assert.equal('starting',worker.phase)
  end)
  it('reports an unknown helper exit without inventing success',function()
    local worker,services=poll_fixture({},nil)
    W.poll(worker,services)
    assert.equal('failed',worker.phase); assert.is_true(worker.exited)
    assert.matches('without a final result',worker.message)
  end)
  it('rejects a reused helper PID',function()
    local worker,services=poll_fixture({{token='generation',phase='ready'}},{pid=2,start='replacement'})
    W.poll(worker,services)
    assert.equal('failed',worker.phase); assert.is_true(worker.exited)
  end)
  it('preserves a published failure after helper exit',function()
    local worker,services=poll_fixture({{token='generation',phase='failed',message='workspace failed; ownership retained'}},nil)
    W.poll(worker,services)
    assert.equal('failed',worker.phase); assert.is_true(worker.exited)
    assert.equal('workspace failed; ownership retained',worker.message)
  end)
  it('recognizes a confirmed stop after helper exit',function()
    local worker,services=poll_fixture({{token='generation',phase='stopped'}},nil)
    W.poll(worker,services)
    assert.equal('stopped',worker.phase); assert.is_true(worker.exited)
  end)
  it('requests cancellation once when startup exceeds its deadline',function()
    local worker,services=poll_fixture({},{pid=2,start='original'})
    worker.started=100
    services.clock={monotonic_ms=function() return 120101 end}
    local calls=0
    services.command=function(value,op,extra)
      assert.equal(worker,value); assert.equal('stop',op); assert.is_false(extra.keep)
      calls=calls+1
    end
    W.poll(worker,services); W.poll(worker,services)
    assert.equal(1,calls); assert.is_true(worker.timed_out)
    assert.equal('failed',worker.phase); assert.matches('cancellation pending',worker.message)
  end)
  it('ignores late ready after timeout but accepts confirmed cleanup',function()
    local events={{token='generation',phase='ready'}}
    local worker,services=poll_fixture(events,{pid=2,start='original'})
    worker.timed_out=true; worker.phase='failed'; worker.message='cancellation pending'
    W.poll(worker,services)
    assert.equal('failed',worker.phase); assert.equal('cancellation pending',worker.message)
    events[2]={token='generation',phase='stopped',message='owned resources released'}
    W.poll(worker,services)
    assert.equal('stopped',worker.phase); assert.equal('owned resources released',worker.message)
  end)
  it('disables the startup deadline after ready',function()
    local events={{token='generation',phase='ready'}}
    local worker,services=poll_fixture(events,{pid=2,start='original'})
    worker.started=100
    services.clock={monotonic_ms=function() return 120101 end}
    services.command=function() error('must not cancel a ready companion') end
    W.poll(worker,services)
    assert.is_nil(worker.started); assert.is_nil(worker.timed_out)
    events[2]={token='generation',phase='stopping'}
    W.poll(worker,services)
    assert.equal('stopping',worker.phase); assert.is_nil(worker.timed_out)
  end)
end)
