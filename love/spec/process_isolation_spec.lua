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
  it('keeps an unrelated adjacent backup while replacing a temporary file',function()
    local Atomic=require('uml.adapters.atomic_file')
    local path=os.tmpname(); os.remove(path)
    local function write(p,text) local f=assert(io.open(p,'wb')); f:write(text); f:close() end
    local function read(p) local f=assert(io.open(p,'rb')); local text=f:read('*a'); f:close(); return text end
    write(path,'original'); write(path..'.fixture-backup','unrelated')
    local ok,err=pcall(function()
      Atomic.write(path,'replacement')
      assert.equals('replacement',read(path))
      assert.equals('unrelated',read(path..'.fixture-backup'))
    end)
    os.remove(path); os.remove(path..'.fixture-backup')
    assert.is_true(ok,err)
  end)
  it('restores the previous target when temporary replacement fails',function()
    local Atomic=require('uml.adapters.atomic_file')
    local path=os.tmpname(); local f=assert(io.open(path,'wb')); f:write('original'); f:close()
    local rename=os.rename
    os.rename=function(source,target)
      if target==path and source:sub(-4)=='.tmp' then return nil,'injected replacement failure' end
      return rename(source,target)
    end
    local ok,err=pcall(Atomic.write,path,'replacement')
    os.rename=rename
    local file=assert(io.open(path,'rb')); local text=file:read('*a'); file:close(); os.remove(path)
    assert.is_false(ok); assert.matches('injected replacement failure',err)
    assert.equals('original',text)
  end)
  it('blocks ownership and command fallbacks and direct shell escape paths',function()
    for _,name in ipairs{'run','spawn_detached','spawn_native','start_attached','owner','owner_native','current_pid','await_exit'} do
      assert.has_error(function() Process[name]() end,'Pure tests forbid native process access: Process.'..name)
    end
    assert.has_error(function() os.execute('herdr') end,'Pure tests forbid native process access: os.execute')
    assert.has_error(function() io.popen('herdr') end,'Pure tests forbid native process access: io.popen')
  end)
end)
