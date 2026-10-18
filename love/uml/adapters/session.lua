-- Restart session file IO only; snapshot semantics stay in application.session.
local E=require('uml.edn')
local Atomic=require('uml.adapters.atomic_file')
local M={}
local function path(root) return root:gsub('\\','/')..'/.uml-viewer/session.edn' end
function M.read(root)
  local f=io.open(path(root),'rb'); if not f then return nil end
  local text=f:read('*a'); f:close()
  local ok,value=pcall(E.parse,text)
  if ok and E.kind(value)=='map' then return value end
  return nil
end
function M.save(root,snapshot)
  local target=path(root); Atomic.ensure_parent(target); Atomic.write(target,E.encode(snapshot)); return snapshot
end
return M
