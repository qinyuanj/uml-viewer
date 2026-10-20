-- Filesystem boundary. POSIX rename atomically replaces an existing file on the
-- same volume. Never delete the destination first: a failure must leave the old
-- contents intact.
local M={}
function M.ensure_parent(path)
  local parent=path:match('^(.*)/'); if not parent or parent=='' then return end
  local lfs=require('lfs')
  local function mkdir(p)
    if lfs.attributes(p,'mode')=='directory' then return end
    local up=p:match('^(.*)/'); if up and up~='' then mkdir(up) end
    assert(lfs.mkdir(p))
  end
  mkdir(parent)
end
-- Optional replace callback is the OS boundary (for failure-injection checks).
function M.write(path,bytes,replace_file)
  -- os.tmpname reserves a unique name; use its basename next to the target to
  -- guarantee a same-volume move. The target is never truncated on failure.
  local reserved=os.tmpname()
  local temporary=path..'.'..reserved:gsub('.*[/\\]','')..'.tmp'
  local f,err=io.open(temporary,'wb')
  if not f then os.remove(reserved); error(err) end
  local ok,result=pcall(function()
    assert(f:write(bytes)); assert(f:flush()); assert(f:close()); f=nil
    local moved,message=(replace_file or os.rename)(temporary,path)
    assert(moved,message)
  end)
  if f then f:close() end
  os.remove(temporary); os.remove(reserved)
  if not ok then error(result,0) end
end
return M
