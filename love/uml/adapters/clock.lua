-- Monotonic clock and sleeping boundary. The value is shared across the viewer,
-- the detached controller and restarted processes on the same machine, so it is
-- a system monotonic clock, never a per-process counter and never os.time.
-- The platform process boundary owns the libc declaration; declaring
-- clock_gettime twice would collide (the first cdef wins in LuaJIT).
local M={}
local windows=package.config:sub(1,1)=='\\'
local okffi,ffi=pcall(require,'ffi')
local tick
function M.monotonic_ms()
  if windows then
    if okffi and tick==nil then
      ffi.cdef[[unsigned long long GetTickCount64(void);]]
      tick=ffi.C.GetTickCount64
    end
    if tick then return tonumber(tick()) end
    return math.floor(os.time()*1000)
  end
  if not okffi then return math.floor(os.time()*1000) end
  local value=require('uml.adapters.posix_process').monotonic_ms()
  return value or math.floor(os.time()*1000)
end
function M.sleep(ms)
  if type(love)=='table' and love.timer and love.timer.sleep then
    love.timer.sleep(ms/1000); return
  end
  if not windows and okffi then
    require('uml.adapters.posix_process').sleep(ms); return
  end
  local deadline=os.clock()+ms/1000
  while os.clock()<deadline do end
end
return M
