-- Durable LOVE process identity document. Pure: the FFI adapter supplies the
-- two FILETIME halves and this module emits exact decimal text. FILETIME is
-- 100ns ticks since 1601 UTC; the full unsigned 64-bit value exceeds both the
-- Lua 5.1 and Lua 5.4 exact integer ranges, so conversion uses decimal limbs.
local M={}
local LIMB=10000000
local MAX32=4294967295
local function valid_half(value)
  return type(value)=='number' and value==math.floor(value) and value>=0 and value<=MAX32
end
local function limbs_of(value)
  local limbs={}
  repeat limbs[#limbs+1]=value%LIMB; value=math.floor(value/LIMB) until value==0
  return limbs
end
local function doubled(limbs)
  local carry=0
  for i=1,#limbs do local value=limbs[i]*2+carry; limbs[i]=value%LIMB; carry=math.floor(value/LIMB) end
  if carry>0 then limbs[#limbs+1]=carry end
end
local function add_small(limbs,value)
  local carry,i=0,1
  while value>0 or carry>0 do
    local part=value%LIMB
    local total=(limbs[i] or 0)+part+carry
    limbs[i]=total%LIMB; carry=math.floor(total/LIMB)
    value=math.floor(value/LIMB); i=i+1
  end
end
function M.filetime(high,low)
  assert(valid_half(high),'FILETIME high half must be an unsigned 32-bit integer')
  assert(valid_half(low),'FILETIME low half must be an unsigned 32-bit integer')
  local limbs=limbs_of(high)
  for _=1,32 do doubled(limbs) end
  add_small(limbs,low)
  local out={tostring(limbs[#limbs])}
  for i=#limbs-1,1,-1 do out[#out+1]=string.format('%07d',limbs[i]) end
  return table.concat(out)
end
local escapes={['"']='\\"',['\\']='\\\\',['\b']='\\b',['\f']='\\f',['\n']='\\n',['\r']='\\r',['\t']='\\t'}
local function json_string(value)
  assert(type(value)=='string','identity text fields must be strings')
  return '"'..value:gsub('[%z\1-\31\\"]',function(c) return escapes[c] or string.format('\\u%04x',c:byte()) end)..'"'
end
function M.encode(record)
  assert(type(record)=='table','identity record must be a table')
  local pid=record.pid
  assert(type(pid)=='number' and pid==math.floor(pid) and pid>=1 and pid<=MAX32,
    'identity pid must be an unsigned 32-bit integer')
  local started=record.started
  assert(type(started)=='string' and started:match('^%d+$'),'identity start time must be unsigned decimal FILETIME ticks')
  assert(record.runtime=='love','identity runtime must be love')
  assert(type(record.cwd)=='string' and #record.cwd>0,'identity cwd must be a non-empty string')
  return '{"pid":'..string.format('%.0f',pid)..',"started":'..json_string(started)
    ..',"runtime":"love","cwd":'..json_string(record.cwd)..'}'
end
return M
