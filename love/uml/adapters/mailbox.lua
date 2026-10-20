-- File IO only. Atomic replace is a durability boundary, not a concurrent-writer lock.
local E=require('uml.edn')
local Protocol=require('uml.domain.mailbox')
local Atomic=require('uml.adapters.atomic_file')
local M={}
local function mtime(path)
  local ok,watch=pcall(function() return require('uml.adapters.platform_watch') end)
  if ok then local found,stamp=pcall(watch.mtime,path); return found and stamp or nil end
  local a=require('lfs').attributes(path)
  return a and (tostring(a.modification)..':'..a.size) or nil
end
function M.open(path,host)
  host=host or {}
  local stamp_of=host.mtime or mtime
  local parse=host.parse or E.parse
  local cached,stamp,known
  local function read()
    local text
    if host.read then text=host.read(path) else
      local f=io.open(path,'rb'); if f then text=f:read('*a'); f:close() end
    end
    local ok,value=false,nil
    if text then ok,value=pcall(parse,text) end
    return Protocol.normalize(ok and value or nil)
  end
  local function write(box)
    (host.write or Atomic.write)(path,E.encode(box))
    -- Never trust a timestamp sampled after our write: an external append may
    -- already have happened. The next pop must read the durable queue afresh.
    known=false
  end
  local api={}
  function api.read() return read() end
  function api.take(seen)
    local current=stamp_of(path)
    if known and current==stamp and #Protocol.pending(cached,seen)==0 then return nil end
    -- Only an unchanged queue with no pending commands may skip reading. Others are
    -- reread for every destructive pop, including commands appended mid-drain.
    local box=read(); cached=box; stamp=current; known=true
    local cmd,remainder=Protocol.take(box,seen)
    if cmd then write(remainder) end
    return cmd
  end
  function api.append(op,extra)
    local cmd,box=Protocol.append(read(),op,extra)
    if not host.write then Atomic.ensure_parent(path) end
    write(box); return cmd
  end
  function api.last_id() return Protocol.last_id(read()) end
  return api
end
function M.new(root) return M.open(root..'/.uml-viewer/to-viewer.edn') end
return M
