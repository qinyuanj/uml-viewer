local E=require('uml.edn')
local Atomic=require('uml.adapters.atomic_file')
local M={}
function M.path(root) return root:gsub('\\','/')..'/.uml-viewer/companion.edn' end
function M.read(root)
  local file=io.open(M.path(root),'rb'); if not file then return nil end
  local text=file:read('*a'); file:close()
  local ok,value=pcall(E.parse,text)
  if not ok then error('Present companion ownership is corrupt or unreadable; file retained',0) end
  return value
end
function M.write(root,record)
  local target=M.path(root); Atomic.ensure_parent(target); Atomic.write(target,E.encode(record)); return record
end
function M.bytes(root)
  local file=io.open(M.path(root),'rb'); if not file then return nil end
  local text=file:read('*a'); file:close(); return text
end
function M.exists(root)
  local file=io.open(M.path(root),'rb'); if not file then return false end; file:close(); return true
end
return M
