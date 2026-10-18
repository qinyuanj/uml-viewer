-- Sibling policy-file boundary: existence, path lookup and atomic writes.
local D=require('uml.application.document')
local Atomic=require('uml.adapters.atomic_file')
local M={}
function M.exists(path)
  local f=io.open(path,'rb')
  if f then f:close(); return true end
  return false
end
function M.path_for(edn_path) return D.policy_path_for(edn_path,M.exists) end
function M.read(path)
  local f=assert(io.open(path,'rb')); local text=f:read('*a'); f:close(); return text
end
function M.write(path,text) Atomic.ensure_parent(path); Atomic.write(path,text) end
return M
