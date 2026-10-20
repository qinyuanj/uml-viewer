-- Platform file timestamp boundary. Both implementations return an opaque
-- stamp string compared for equality only; sub-second resolution is required
-- so two edits inside one second still reload.
local M={}
local windows=package.config:sub(1,1)=='\\'
function M.load()
  if windows then return require('uml.adapters.file_watch') end
  return require('uml.adapters.posix_watch')
end
return M
