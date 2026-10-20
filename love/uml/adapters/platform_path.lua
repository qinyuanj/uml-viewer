-- Platform path identity and directory enumeration. Selection is made once
-- from the host Lua runtime; consumers keep their existing contracts
-- (canonical/entries), so callers never branch on the platform.
local M={}
local windows=package.config:sub(1,1)=='\\'
function M.windows() return windows end
function M.load()
  if windows then return require('uml.adapters.windows_path') end
  return require('uml.adapters.posix_path')
end
return M
