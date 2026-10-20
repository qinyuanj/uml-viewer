-- Monotonic clock and sleeping boundary. The value is shared across the viewer,
-- the detached controller and restarted processes on the same machine, so it is
-- a system monotonic clock, never a per-process counter and never os.time.
-- The pure suite runs under Lua 5.4, where no FFI exists; there the fallbacks
-- keep pure logic testable without pretending they are production clocks.
local M={}
local okffi=pcall(require,'ffi')
local function system()
  if okffi then return require('uml.adapters.posix_process') end
end
function M.monotonic_ms()
  local process=system()
  local value=process and process.monotonic_ms()
  return value or math.floor(os.time()*1000)
end
function M.sleep(ms)
  if type(love)=='table' and love.timer and love.timer.sleep then
    love.timer.sleep(ms/1000); return
  end
  local process=system()
  if process then process.sleep(ms); return end
  local deadline=os.clock()+ms/1000
  while os.clock()<deadline do end
end
return M
