-- Append-only exception logging belongs at the filesystem boundary.
local M={default_path='uml-viewer-log.txt'}
function M.exception(message,context,path)
  local trace=debug.traceback(tostring(message),2)
  pcall(function()
    local f=assert(io.open(path or M.default_path,'a'))
    f:write(os.date('!%Y-%m-%dT%H:%M:%SZ'),context and (' '..context) or '', '\n',trace,'\n\n')
    f:close()
  end)
  return trace
end
function M.handler(path,context)
  return function(message) return M.exception(message,context or 'uncaught on viewer',path) end
end
return M
