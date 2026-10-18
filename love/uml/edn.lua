-- EDN protocol values: Lua strings/numbers/booleans; nil_value preserves EDN nil.
-- keyword(name), symbol(name), vector(items), map({{key,value}, ...}) are tagged.
-- Access maps through get/assoc: keys compare structurally, including vectors.
-- parse(text) and encode(value) throw on invalid/unsupported input; equal(a,b)
-- compares EDN semantics (map ordering is irrelevant). Symbols preserve legacy policy.
local M = {}
local tags = {}
for _, name in ipairs({'keyword', 'symbol', 'vector', 'map', 'nil'}) do tags[name] = {kind=name} end
M.nil_value = setmetatable({}, tags['nil'])
function M.keyword(name) return setmetatable({name=name}, tags.keyword) end
function M.symbol(name) return setmetatable({name=name}, tags.symbol) end
function M.vector(items) return setmetatable(items or {}, tags.vector) end
function M.map(entries) return setmetatable(entries or {}, tags.map) end
function M.kind(value)
  local mt = type(value) == 'table' and getmetatable(value)
  return mt and mt.kind or type(value)
end
function M.equal(a, b)
  local kind = M.kind(a)
  if kind ~= M.kind(b) then return false end
  if kind == 'keyword' or kind == 'symbol' then return a.name == b.name end
  if kind == 'nil' then return true end
  if kind == 'vector' then
    if #a ~= #b then return false end
    for i=1,#a do if not M.equal(a[i], b[i]) then return false end end
    return true
  end
  if kind == 'map' then
    if #a ~= #b then return false end
    for _, entry in ipairs(a) do
      local value, found = M.get(b, entry[1])
      if not found or not M.equal(entry[2], value) then return false end
    end
    return true
  end
  return a == b
end
function M.get(map, key)
  for _, entry in ipairs(map) do if M.equal(entry[1], key) then return entry[2], true end end
  return nil, false
end
function M.assoc(map, key, value)
  assert(value ~= nil, 'use nil_value for EDN nil')
  for _, entry in ipairs(map) do if M.equal(entry[1], key) then entry[2]=value; return map end end
  map[#map+1]={key,value}
  return map
end
local function utf8(code)
  if code < 128 then return string.char(code) end
  if code < 2048 then return string.char(192+math.floor(code/64),128+code%64) end
  if code < 65536 then return string.char(224+math.floor(code/4096),128+math.floor(code/64)%64,128+code%64) end
  return string.char(240+math.floor(code/262144),128+math.floor(code/4096)%64,128+math.floor(code/64)%64,128+code%64)
end
function M.parse(text)
  local p, n = 1, #text
  local function fail(message) error('EDN byte '..p..': '..message, 0) end
  local function skip()
    while p <= n do
      local c=text:sub(p,p)
      if c:match('[%s,]') then p=p+1
      elseif c == ';' then p=(text:find('\n',p,true) or n)+1
      else break end
    end
  end
  local read
  read = function()
    skip()
    local c=text:sub(p,p)
    if c == '"' then
      p=p+1
      local out={}
      local escapes={n='\n',r='\r',t='\t',b='\b',f='\f',['"']='"',['\\']='\\'}
      while p <= n do
        c=text:sub(p,p); p=p+1
        if c == '"' then return table.concat(out) end
        if c == '\\' then
          c=text:sub(p,p); p=p+1
          if c == 'u' then
            local hex=text:sub(p,p+3)
            if not hex:match('^%x%x%x%x$') then fail('invalid Unicode escape') end
            local code=tonumber(hex,16); p=p+4
            if code >= 55296 and code <= 56319 then
              if text:sub(p,p+1) ~= '\\u' then fail('missing low surrogate') end
              local lowhex=text:sub(p+2,p+5)
              local low=lowhex:match('^%x%x%x%x$') and tonumber(lowhex,16)
              if not low or low < 56320 or low > 57343 then fail('invalid low surrogate') end
              code=65536+(code-55296)*1024+low-56320; p=p+6
            elseif code >= 56320 and code <= 57343 then fail('unexpected low surrogate') end
            out[#out+1]=utf8(code)
          elseif escapes[c] then out[#out+1]=escapes[c]
          else fail('invalid string escape') end
        else out[#out+1]=c end
      end
      fail('unterminated string')
    elseif c == '[' or c == '{' then
      p=p+1
      local close=c == '[' and ']' or '}'
      local result=c == '[' and M.vector() or M.map()
      skip()
      while text:sub(p,p) ~= close do
        if p > n then fail('unterminated collection') end
        local key=read()
        if c == '[' then result[#result+1]=key
        else
          skip()
          if text:sub(p,p) == close then fail('map requires key/value pairs') end
          local value=read()
          local _, found=M.get(result,key)
          if found then fail('duplicate map key') end
          M.assoc(result,key,value)
        end
        skip()
      end
      p=p+1
      return result
    end
    local start=p
    while p <= n and not text:sub(p,p):match('[%s,%[%]{}";]') do p=p+1 end
    local token=text:sub(start,p-1)
    if token == '' then fail('expected value') end
    if token == 'nil' then return M.nil_value end
    if token == 'true' then return true end
    if token == 'false' then return false end
    if token:sub(1,1) == ':' and #token > 1 then return M.keyword(token:sub(2)) end
    local number=tonumber(token)
    if number then return number end
    if token:match('^[%a_*+!/?<>=.$%%&%-][%w_*+!/?<>=.$%%&:%-]*$') then return M.symbol(token) end
    fail('unsupported token '..token)
  end
  local value=read(); skip()
  if p <= n then fail('trailing input') end
  return value
end
local escapes={['"']='\\"',['\\']='\\\\',['\n']='\\n',['\r']='\\r',['\t']='\\t',['\b']='\\b',['\f']='\\f'}
function M.encode(value)
  local kind=M.kind(value)
  if kind == 'nil' then return 'nil' end
  if kind == 'keyword' then return ':'..value.name end
  if kind == 'symbol' then return value.name end
  if kind == 'boolean' then return tostring(value) end
  if kind == 'number' then
    assert(value == value and value ~= math.huge and value ~= -math.huge, 'non-finite EDN number')
    return string.format('%.17g',value)
  end
  if kind == 'string' then
    return '"'..value:gsub('[%z\1-\31\\"]',function(c) return escapes[c] or string.format('\\u%04x',c:byte()) end)..'"'
  end
  local out={}
  if kind == 'vector' then
    for i,item in ipairs(value) do out[i]=M.encode(item) end
    return '['..table.concat(out,' ')..']'
  end
  if kind == 'map' then
    for i,entry in ipairs(value) do out[i]=M.encode(entry[1])..' '..M.encode(entry[2]) end
    return '{'..table.concat(out,' ')..'}'
  end
  error('unsupported EDN value: '..kind)
end
return M
