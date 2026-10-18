local edn=require('uml.edn')
local atomic=require('uml.adapters.atomic_file')
local M={}
function M.read(path)
  local f=assert(io.open(path,'rb')); local bytes=f:read('*a'); f:close()
  return edn.parse(bytes)
end
function M.write(path,value) return atomic.write(path,edn.encode(value)..'\n') end
return M
