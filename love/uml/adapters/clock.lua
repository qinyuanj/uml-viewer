-- Monotonic clock and sleeping boundary. Under LOVE, GetTickCount64 is shared
-- across the detached controller and viewer restarts; os.time is only a
-- fallback outside the LOVE runtime and is never used for tests.
local M={}
local tick_count
function M.monotonic_ms()
  if tick_count==nil then
    local ok,ffi=pcall(require,'ffi')
    if ok then ffi.cdef[[unsigned long long GetTickCount64(void);]]; tick_count=ffi.C end
    tick_count=tick_count or false
  end
  if tick_count then return tonumber(tick_count.GetTickCount64()) end
  return math.floor(os.time()*1000)
end
function M.sleep(ms)
  if type(love)=='table' and love.timer and love.timer.sleep then
    love.timer.sleep(ms/1000)
  else
    local deadline=os.clock()+ms/1000
    while os.clock()<deadline do end
  end
end
return M
