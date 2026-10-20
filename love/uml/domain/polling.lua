-- Monotonic clock boundary. Call before any discovery, stat or read.
local M={}
function M.new(clock)
  local checked
  return {due=function(force)
    local now=clock()
    if not force and checked and now<checked+.250 then return false end
    checked=now
    return true
  end}
end
return M
