-- Minimal JSON boundary for Herdr CLI output and detached-process scripts.
-- Objects are string-keyed tables; arrays carry a marker so [] stays distinct
-- from {}. `null` is a sentinel because Lua nil cannot express a present null.
local M={}
M.null=setmetatable({},{__tostring=function() return 'null' end})
local array_mt={__json_array=true}
function M.array(items) return setmetatable(items or {},array_mt) end
function M.is_array(value) return type(value)=='table' and getmetatable(value)==array_mt end

local escapes={['"']='\\"',['\\']='\\\\',['\b']='\\b',['\f']='\\f',['\n']='\\n',['\r']='\\r',['\t']='\\t'}

function M.decode(text)
  assert(type(text)=='string','JSON input must be text')
  local position,size=1,#text
  local function fail(message) error('JSON byte '..position..': '..message,0) end
  local function skip()
    position=text:find('[^ \t\r\n]',position) or size+1
  end
  local value
  local function string_value()
    position=position+1
    local out={}
    while position<=size do
      local char=text:sub(position,position)
      if char=='"' then position=position+1; return table.concat(out) end
      if char=='\\' then
        local escape=text:sub(position+1,position+1)
        if escape=='u' then
          local hex=text:sub(position+2,position+5)
          if not hex:match('^%x%x%x%x$') then fail('bad unicode escape') end
          local code=tonumber(hex,16); position=position+6
          if code>=0xD800 and code<=0xDBFF then
            local low=text:sub(position,position+1)
            if low~='\\u' then fail('missing low surrogate') end
            local low_hex=text:sub(position+2,position+5)
            if not low_hex:match('^%x%x%x%x$') then fail('bad low surrogate') end
            local low_code=tonumber(low_hex,16)
            if low_code<0xDC00 or low_code>0xDFFF then fail('bad low surrogate') end
            code=0x10000+(code-0xD800)*1024+(low_code-0xDC00); position=position+6
          end
          if code<0x80 then out[#out+1]=string.char(code)
          elseif code<0x800 then out[#out+1]=string.char(0xC0+math.floor(code/64),0x80+code%64)
          elseif code<0x10000 then
            out[#out+1]=string.char(0xE0+math.floor(code/4096),0x80+math.floor(code/64)%64,0x80+code%64)
          else
            out[#out+1]=string.char(0xF0+math.floor(code/262144),0x80+math.floor(code/4096)%64,
              0x80+math.floor(code/64)%64,0x80+code%64)
          end
        else
          local decoded={['"']='"',['\\']='\\',['/']='/',b='\b',f='\f',n='\n',r='\r',t='\t'}
          if not decoded[escape] then fail('bad escape') end
          out[#out+1]=decoded[escape]; position=position+2
        end
      else
        if char:byte()<32 then fail('control character in string') end
        out[#out+1]=char; position=position+1
      end
    end
    fail('unterminated string')
  end
  local function number_value()
    local start=position
    while position<=size and text:sub(position,position):match('[-+0-9.eE]') do position=position+1 end
    local literal=text:sub(start,position-1)
    if not literal:match('^-?%d+%.?%d*[eE]?[-+]?%d*$') then fail('bad number') end
    local number=tonumber(literal)
    if not number then fail('bad number') end
    return number
  end
  local function literal(word,v)
    if text:sub(position,position+#word-1)~=word then fail('expected '..word) end
    position=position+#word; return v
  end
  local function read_array()
    position=position+1; local items=M.array{}
    skip(); if text:sub(position,position)==']' then position=position+1; return items end
    while true do
      items[#items+1]=value()
      skip(); local char=text:sub(position,position)
      if char==',' then position=position+1; skip()
      elseif char==']' then position=position+1; return items
      else fail('expected , or ]') end
    end
  end
  local function read_object()
    position=position+1; local object={}
    skip(); if text:sub(position,position)=='}' then position=position+1; return object end
    while true do
      skip()
      if text:sub(position,position)~='"' then fail('expected object key') end
      local key=string_value(); skip()
      if text:sub(position,position)~=':' then fail('expected :') end
      position=position+1; object[key]=value(); skip()
      local char=text:sub(position,position)
      if char==',' then position=position+1
      elseif char=='}' then position=position+1; return object
      else fail('expected , or }') end
    end
  end
  value=function()
    skip()
    local char=text:sub(position,position)
    if char=='{' then return read_object() end
    if char=='[' then return read_array() end
    if char=='"' then return string_value() end
    if char=='-' or char:match('%d') then return number_value() end
    if char=='t' then return literal('true',true) end
    if char=='f' then return literal('false',false) end
    if char=='n' then return literal('null',M.null) end
    fail('unexpected input')
  end
  local result=value(); skip()
  if position<=size then fail('trailing input') end
  return result
end

local function encode_string(value)
  return '"'..value:gsub('[%z\1-\31\\"]',function(c) return escapes[c] or string.format('\\u%04x',c:byte()) end)..'"'
end
function M.encode(value)
  local kind=type(value)
  if value==M.null then return 'null' end
  if kind=='nil' then return 'null' end
  if kind=='boolean' then return value and 'true' or 'false' end
  if kind=='number' then
    if value==math.floor(value) then return string.format('%.0f',value) end
    return string.format('%.14g',value)
  end
  if kind=='string' then return encode_string(value) end
  if kind=='table' then
    local out={}
    if M.is_array(value) then
      for i=1,#value do out[#out+1]=M.encode(value[i]) end
      return '['..table.concat(out,',')..']'
    end
    for key,item in pairs(value) do
      if type(key)~='string' then error('JSON object keys must be strings',0) end
      out[#out+1]=encode_string(key)..':'..M.encode(item)
    end
    return '{'..table.concat(out,',')..'}'
  end
  error('unsupported JSON value: '..kind,0)
end
return M
