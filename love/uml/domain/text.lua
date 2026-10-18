-- Pure argv/text encodings shared by the process and Herdr boundaries.
-- Base64 and UTF-16LE are needed for PowerShell -EncodedCommand; shell/ps
-- quoting preserves literal argv when a pane shell must parse it.
local M={}
local alphabet='ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/'

function M.base64(bytes)
  local out={}
  for i=1,#bytes,3 do
    local a,b,c=bytes:byte(i,i+2); b=b or 0; c=c or 0
    local value=a*65536+b*256+c
    out[#out+1]=alphabet:sub(math.floor(value/262144)%64+1,math.floor(value/262144)%64+1)
    out[#out+1]=alphabet:sub(math.floor(value/4096)%64+1,math.floor(value/4096)%64+1)
    out[#out+1]=i+1<=#bytes and alphabet:sub(math.floor(value/64)%64+1,math.floor(value/64)%64+1) or '='
    out[#out+1]=i+2<=#bytes and alphabet:sub(value%64+1,value%64+1) or '='
  end
  return table.concat(out)
end

-- UTF-8 to UTF-16LE with surrogate pairs for astral text.
function M.utf16le(text)
  local out={}
  local function unit(code) out[#out+1]=string.char(code%256,math.floor(code/256)%256) end
  local i=1
  while i<=#text do
    local first=text:byte(i)
    local code,length
    if first<0x80 then code,length=first,1
    elseif first<0xE0 then code,length=first-0xC0,2
    elseif first<0xF0 then code,length=first-0xE0,3
    else code,length=first-0xF0,4 end
    if length>1 then
      code=code%2^(6*(length-1)-2)
      for offset=1,length-1 do code=code*64+(text:byte(i+offset)-0x80) end
    end
    if code<=0xFFFF then unit(code)
    else code=code-0x10000; unit(0xD800+math.floor(code/1024)); unit(0xDC00+code%1024) end
    i=i+length
  end
  return table.concat(out)
end

function M.encoded(text) return M.base64(M.utf16le(text)) end
function M.ps_word(text) return "'"..tostring(text):gsub("'","''").."'" end
function M.shell_word(text) return "'"..tostring(text):gsub("'","'\"'\"'").."'" end

-- Windows CommandLineToArgvW quoting: literal argv, never shell reparse.
function M.win_quote(text)
  text=tostring(text)
  if text=='' then return '""' end
  if not text:find('[%s"]') then return text end
  local out={'"'}
  local backslashes=0
  for i=1,#text do
    local char=text:sub(i,i)
    if char=='\\' then backslashes=backslashes+1
    elseif char=='"' then out[#out+1]=string.rep('\\',backslashes*2+1)..'"'; backslashes=0
    else out[#out+1]=string.rep('\\',backslashes)..char; backslashes=0 end
  end
  out[#out+1]=string.rep('\\',backslashes*2)..'"'
  return table.concat(out)
end
return M
