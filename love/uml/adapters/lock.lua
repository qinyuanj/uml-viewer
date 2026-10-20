-- OS-owned file locks selected once per runtime; both implementations provide
-- singleton/gate semantics whose lock dies with the holding process.
local M={}
local windows=package.config:sub(1,1)=='\\'
function M.load()
  if windows then return require('uml.adapters.os_lock') end
  return require('uml.adapters.posix_lock')
end
function M.try_acquire(path) return M.load().try_acquire(path) end
function M.acquire(path,options) return M.load().acquire(path,options) end
function M.release(handle) return M.load().release(handle) end
return M
