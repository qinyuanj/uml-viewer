-- Process boundary selected once per runtime. Every consumer keeps the same
-- contract: literal argv, data results, exact creation identity. Windows uses
-- PowerShell 7/FFI, POSIX uses posix_spawn and /proc. Neither platform's
-- internal representation leaks into callers.
--
-- The facade forwards to the selected implementation. Tests replace individual
-- entries on the facade, and a raw assignment shadows the forwarded value, so
-- the existing injection boundary keeps working unchanged.
local M={}
local Text=require('uml.domain.text')
local windows=package.config:sub(1,1)=='\\'
local implementation
local function selected()
  if not implementation then
    implementation=windows and require('uml.adapters.windows_process') or require('uml.adapters.posix_process')
  end
  return implementation
end
function M.windows() return windows end
function M.load() return selected() end
-- Command-line quoting is a pure string contract, identical on both platforms
-- so the Windows boundary's behaviour stays directly testable off Windows.
M.win_quote=Text.win_quote
M.ps_word=Text.ps_word
setmetatable(M,{__index=function(_,key)
  local value=selected()[key]
  if type(value)=='function' then return value end
  return value
end})
return M
