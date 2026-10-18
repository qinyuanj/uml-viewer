-- File IO only. Atomic replace is a durability boundary, not a concurrent-writer lock.
local E=require('uml.edn')
local Protocol=require('uml.domain.mailbox')
local Atomic=require('uml.adapters.atomic_file')
local M={}
function M.open(path)
  local function read()
    local f=io.open(path,'rb'); if not f then return Protocol.normalize() end
    local text=f:read('*a'); f:close(); local ok,value=pcall(E.parse,text)
    return Protocol.normalize(ok and value or nil)
  end
  local api={}
  function api.read() return read() end
  function api.take(seen)
    local cmd,box=Protocol.take(read(),seen)
    if cmd then Atomic.write(path,E.encode(box)) end
    return cmd
  end
  function api.append(op,extra)
    local cmd,box=Protocol.append(read(),op,extra); Atomic.write(path,E.encode(box)); return cmd
  end
  function api.last_id() return Protocol.last_id(read()) end
  return api
end
function M.new(root) return M.open(root..'/.uml-viewer/to-viewer.edn') end
return M
