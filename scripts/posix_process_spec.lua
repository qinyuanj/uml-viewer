-- Real OS behaviour of the POSIX process boundary. Runs only under the native
-- harness (LuaJIT); the pure suite forbids process access.
local P=require('uml.adapters.posix_process')

describe('POSIX process boundary',function()
  it('passes argv literally, including spaces, Unicode and shell metacharacters',function()
    local words={'帶 空格','a$(touch injected)b','半角 ; & | > <','quote"inside'}
    local result=P.run({'/usr/bin/python3','-c','import json,sys;print(json.dumps(sys.argv[1:]))',
      table.unpack(words)})
    assert.are.equal(0,result.exit)
    local decoded=require('uml.adapters.json').decode(result.out)
    assert.are.same(words,decoded)
  end)

  it('reports the real exit code and both streams',function()
    local result=P.run({'/bin/sh','-c','printf out; printf err >&2; exit 23'})
    assert.are.equal(23,result.exit)
    assert.are.equal('out',result.out)
    assert.are.equal('err',result.err)
  end)

  it('times out a bounded command and reports it as timed out',function()
    local result=P.run({'/bin/sleep','10'},300)
    assert.is_true(result.timeout)
    assert.is_nil(result.exit)
  end)

  it('reads a creation identity shared by every observer',function()
    local identity=P.owner_native(P.current_pid())
    assert.are.equal(P.current_pid(),identity.pid)
    assert.is_string(identity.start)
    local again=P.owner_native(tostring(identity.pid))
    assert.are.equal(identity.start,again.start)
  end)

  it('returns nil for a PID that is not running',function()
    assert.is_nil(P.owner_native(4194304))
  end)

  it('rejects an invalid PID instead of guessing',function()
    assert.has_error(function() P.owner_native('not-a-pid') end)
  end)

  it('spawns a detached process and reports its identity',function()
    local log=os.tmpname()
    local error_log=log..'.err'
    local owner=P.spawn_detached('/bin/sleep',{'2'},'/tmp',log,error_log)
    assert.is_number(owner.pid)
    assert.is_string(owner.start)
    assert.are.equal(owner.start,P.owner_native(owner.pid).start)
    os.remove(log); os.remove(error_log)
  end)

  it('removes only the named variables from the child environment',function()
    local present=P.run({'/bin/sh','-c','printf %s "$HOME"'},5000)
    assert.are.equal(0,present.exit)
    assert.are.equal(os.getenv('HOME'),present.out)
    local removed=P.run({'/bin/sh','-c','printf %s "${HOME:-unset}"'},5000,{'HOME'})
    assert.are.equal(0,removed.exit)
    assert.are.equal('unset',removed.out)
  end)
end)
