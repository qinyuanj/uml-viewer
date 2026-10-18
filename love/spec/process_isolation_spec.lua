local Process=require('uml.adapters.process')
local Herdr=require('uml.adapters.herdr')

describe('pure suite process isolation',function()
  it('blocks production Herdr launch when the high-level injection is bypassed',function()
    local execute,popen=os.execute,io.popen
    local calls=0
    local function sentinel() calls=calls+1; error('OS sentinel reached',0) end
    os.execute=sentinel; io.popen=sentinel
    local ok,err=pcall(function()
      local backend=Herdr.new({}, {selected={path='herdr'}})
      assert.has_error(function() backend.launch_server('uml-viewer-isolation','C:/fixture') end,
        'Pure tests forbid native process access: Process.spawn_detached')
      assert.equals(0,calls)
    end)
    os.execute=execute; io.popen=popen
    assert.is_true(ok,err)
  end)
  it('blocks ownership and command fallbacks and direct shell escape paths',function()
    for _,name in ipairs{'run','spawn_detached','pwsh','start_attached','owner','owner_native','current_pid','await_exit'} do
      assert.has_error(function() Process[name]() end,'Pure tests forbid native process access: Process.'..name)
    end
    assert.has_error(function() os.execute('herdr') end,'Pure tests forbid native process access: os.execute')
    assert.has_error(function() io.popen('herdr') end,'Pure tests forbid native process access: io.popen')
  end)
end)
