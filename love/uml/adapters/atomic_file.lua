-- Filesystem boundary. Windows CRT rename cannot overwrite, so use .NET File.Move
-- with overwrite via PowerShell 7. It performs one MoveFileEx replacement on the
-- same volume. Never delete the destination first. POSIX uses rename directly.
local M={}
local windows=package.config:sub(1,1) == '\\'
local function base64(bytes)
  local alphabet='ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/'
  local out={}
  for i=1,#bytes,3 do
    local a,b,c=bytes:byte(i,i+2); b=b or 0; c=c or 0
    local value=a*65536+b*256+c
    out[#out+1]=alphabet:sub(math.floor(value/262144)%64+1,math.floor(value/262144)%64+1)
    out[#out+1]=alphabet:sub(math.floor(value/4096)%64+1,math.floor(value/4096)%64+1)
    out[#out+1]=i+1 <= #bytes and alphabet:sub(math.floor(value/64)%64+1,math.floor(value/64)%64+1) or '='
    out[#out+1]=i+2 <= #bytes and alphabet:sub(value%64+1,value%64+1) or '='
  end
  return table.concat(out)
end
local function replace(source,target)
  if not windows then return os.rename(source,target) end
  local function path(value)
    return "[Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('"..base64(value).."'))"
  end
  local script="$ErrorActionPreference='Stop'; [System.IO.File]::Move("..path(source)..','..path(target)..',$true)'
  -- EncodedCommand avoids cmd.exe expansion of percent signs or metacharacters
  -- in filenames. The script is ASCII; paths are base64-encoded UTF-8.
  local utf16=script:gsub('.',function(c) return c..'\0' end)
  local command='pwsh -NoProfile -NonInteractive -EncodedCommand '..base64(utf16)
  local ok, _, code=os.execute(command)
  if ok == true or ok == 0 then return true end
  return nil,'atomic replacement failed (PowerShell exit '..tostring(code or ok)..')'
end
function M.ensure_parent(path)
  local parent=path:match('^(.*)[/\\]'); if not parent or parent=='' then return end
  if windows then
    local script="$ErrorActionPreference='Stop'; [void][IO.Directory]::CreateDirectory([Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('"..base64(parent).."')))"
    local ok,_,code=os.execute('pwsh -NoProfile -NonInteractive -EncodedCommand '..base64(script:gsub('.',function(c) return c..'\0' end)))
    assert(ok==true or ok==0,'mailbox directory creation failed: '..tostring(code or ok))
  else
    local lfs=require('lfs')
    local function mkdir(p)
      if lfs.attributes(p,'mode')=='directory' then return end
      local up=p:match('^(.*)/'); if up and up~='' then mkdir(up) end
      assert(lfs.mkdir(p))
    end
    mkdir(parent)
  end
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
    local moved,message=(replace_file or replace)(temporary,path)
    assert(moved,message)
  end)
  if f then f:close() end
  os.remove(temporary); os.remove(reserved)
  if not ok then error(result,0) end
end
return M
