-- External process boundary. Windows uses PowerShell 7 with ArgumentList so
-- argv stays literal (no cmd reparse). Output/status are returned as data.
-- Creation identity is decimal .NET ticks, compared exactly as text.
local JSON=require('uml.adapters.json')
local Text=require('uml.domain.text')
local unpack=require('uml.compat').unpack
local M={}
local windows=package.config:sub(1,1)=='\\'

function M.windows() return windows end
M.base64=Text.base64
M.utf16le=Text.utf16le
M.encoded=Text.encoded
M.ps_word=Text.ps_word
M.win_quote=Text.win_quote

function M.pwsh(script)
  local pipe=io.popen('pwsh.exe -NoProfile -NonInteractive -EncodedCommand '..M.encoded(script),'r')
  if not pipe then error('PowerShell is required for the process boundary',0) end
  local output=pipe:read('*a')
  pipe:close()
  return output
end

local ps_word=Text.ps_word

-- ArgumentList preserves each element literally; only the PowerShell literal
-- needs quoting here. Start-Process re-joins a command line, so it applies
-- win_quote separately.
local function ps_argv(argv)
  local out={}
  for _,value in ipairs(argv or {}) do out[#out+1]=ps_word(value) end
  return table.concat(out,',')
end
local function win_argv(argv)
  local out={}
  for _,value in ipairs(argv or {}) do out[#out+1]=ps_word(M.win_quote(value)) end
  return table.concat(out,',')
end

function M.current_pid()
  local ok,ffi=pcall(require,'ffi')
  if not ok then error('Current process identity requires the LOVE FFI boundary',0) end
  ffi.cdef[[unsigned long GetCurrentProcessId(void);]]
  return tonumber(ffi.C.GetCurrentProcessId())
end

local native_api
local function process_api()
  if native_api~=nil then return native_api or nil end
  local ok,ffi=pcall(require,'ffi')
  if not ok then native_api=false; return nil end
  if not pcall(ffi.typeof,'UML_FILETIME') then
    ffi.cdef[[typedef struct { unsigned long dwLowDateTime; unsigned long dwHighDateTime; } UML_FILETIME;]]
  end
  ffi.cdef[[
    void* OpenProcess(unsigned long,int,unsigned long);
    int GetProcessTimes(void*,UML_FILETIME*,UML_FILETIME*,UML_FILETIME*,UML_FILETIME*);
    unsigned long WaitForSingleObject(void*,unsigned long);
    int CloseHandle(void*);
    unsigned long GetLastError(void);
  ]]
  native_api=ffi; return ffi
end

-- Fast creation identity without spawning PowerShell. Used by the detached
-- controller's 100ms loop; only a positively absent PID returns nil.
function M.owner_native(pid)
  if type(pid)=='string' and pid:match('^[1-9]%d*$') then pid=tonumber(pid) end
  if type(pid)~='number' or pid~=math.floor(pid) or pid<1 then
    error('Invalid owned process PID: '..tostring(pid),0)
  end
  local ffi=process_api()
  if not ffi then return M.owner(pid) end
  local handle=ffi.C.OpenProcess(0x101000,0,pid) -- QUERY_LIMITED_INFORMATION|SYNCHRONIZE
  if handle==nil then
    local code=tonumber(ffi.C.GetLastError())
    if code==87 or code==1168 then return nil end
    error('Could not open process '..pid..' (Windows error '..code..')',0)
  end
  -- A terminated process can still be opened while another process holds a
  -- handle to it. Only the signaled state proves that it is no longer running.
  local signaled=tonumber(ffi.C.WaitForSingleObject(handle,0))
  if signaled==0 then ffi.C.CloseHandle(handle); return nil end -- WAIT_OBJECT_0
  if signaled~=258 then -- WAIT_TIMEOUT means still running
    local code=tonumber(ffi.C.GetLastError())
    ffi.C.CloseHandle(handle)
    error('Could not query process '..pid..' state (Windows error '..code..')',0)
  end
  local creation=ffi.new('UML_FILETIME[1]')
  local scratch=ffi.new('UML_FILETIME[1]')
  local ok=ffi.C.GetProcessTimes(handle,creation,scratch,scratch,scratch)
  ffi.C.CloseHandle(handle)
  if ok==0 then error('Could not read process creation identity for PID '..pid,0) end
  local Identity=require('uml.domain.identity')
  local ticks=Identity.filetime(tonumber(creation[0].dwHighDateTime),tonumber(creation[0].dwLowDateTime))
  return {pid=pid,start=Identity.dotnet_ticks(ticks)}
end

function M.run(argv,timeout_ms,env_remove)
  timeout_ms=timeout_ms or 10000
  local script={
    "$ErrorActionPreference='Stop'",
    '$psi=[Diagnostics.ProcessStartInfo]::new('..ps_word(argv[1])..')',
    '$psi.UseShellExecute=$false',
    '$psi.RedirectStandardInput=$true;$psi.RedirectStandardOutput=$true;$psi.RedirectStandardError=$true',
    'foreach($a in @('..ps_argv({unpack(argv,2)})..')){[void]$psi.ArgumentList.Add([string]$a)}',
    'foreach($n in @('..ps_argv(env_remove or {})..')){[void]$psi.Environment.Remove([string]$n)}',
    'try{$p=[Diagnostics.Process]::Start($psi)}catch{@{exit=$null;out="";err=$_.Exception.Message;timeout=$false}|ConvertTo-Json -Compress;exit 0}',
    '$p.StandardInput.Close()',
    '$o=$p.StandardOutput.ReadToEndAsync();$e=$p.StandardError.ReadToEndAsync()',
    '$done=$p.WaitForExit([int]'..math.floor(tonumber(timeout_ms))..')',
    'if(-not $done){try{$p.Kill()}catch{}}',
    '$result=@{exit=$(if($done){$p.ExitCode}else{$null});out=$o.GetAwaiter().GetResult().Trim();err=$(if($done){$e.GetAwaiter().GetResult().Trim()}else{"process timed out"});timeout=(-not $done)}',
    '$result|ConvertTo-Json -Compress',
  }
  local output=M.pwsh(table.concat(script,';'))
  local ok,value=pcall(JSON.decode,output)
  if not ok or type(value)~='table' then
    return {exit=nil,out='',err=output~='' and output or 'process boundary failed',timeout=false}
  end
  local exit=value.exit; if exit==JSON.null then exit=nil end; return {exit=exit,out=value.out~=JSON.null and (value.out or '') or '',err=value.err~=JSON.null and (value.err or '') or '',timeout=value.timeout==true}
end

function M.owner(pid)
  if type(pid)=='string' and pid:match('^[1-9]%d*$') then pid=tonumber(pid) end
  if type(pid)~='number' or pid~=math.floor(pid) or pid<1 then
    error('Invalid owned process PID: '..tostring(pid),0)
  end
  local script="$ErrorActionPreference='Stop';try{$p=[Diagnostics.Process]::GetProcessById("..string.format('%.0f',pid)
    ..")}catch [ArgumentException]{[Console]::Write('null');exit 0};"
    .."if($p.HasExited){[Console]::Write('null');exit 0};"
    .."@{pid=$p.Id;start=$p.StartTime.ToUniversalTime().Ticks.ToString();name=$p.ProcessName}|ConvertTo-Json -Compress"
  local output=M.pwsh(script):gsub('%s+$','')
  if output=='null' then return nil end
  local ok,value=pcall(JSON.decode,output)
  if not ok or type(value)~='table' or type(value.pid)~='number' or type(value.start)~='string'
      or type(value.name)~='string' then
    error('Could not read process creation identity for PID '..tostring(pid)..': '..output,0)
  end
  return {pid=value.pid,start=value.start,name=value.name}
end

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

local function start_script(executable,argv,cwd,log_path,error_log,hidden,result_path)
  local report='@{pid=$p.Id;start=$p.StartTime.ToUniversalTime().Ticks.ToString();name=$p.ProcessName}|ConvertTo-Json -Compress'
  if result_path then report=report..'|Set-Content -LiteralPath '..ps_word(result_path)..' -Encoding utf8' end
  local script={
    "$ErrorActionPreference='Stop'",
    '$params=@{FilePath='..ps_word(executable)..';PassThru=$true}',
    '$argv=@('..win_argv(argv or {})..')',
    '$params.ArgumentList=$argv',
    (hidden and '$params.WindowStyle="Hidden"' or '$null'),
    (cwd and cwd~='' and ('$params.WorkingDirectory='..ps_word(cwd))) or '$null',
    (log_path and ('$params.RedirectStandardOutput='..ps_word(log_path))) or '$null',
    (error_log and ('$params.RedirectStandardError='..ps_word(error_log))) or '$null',
    'try{$p=Start-Process @params}catch{@{error=$_.Exception.Message}|ConvertTo-Json -Compress;exit 1}',
    report,
  }
  local filtered={}
  for _,line in ipairs(script) do if line~='$null' then filtered[#filtered+1]=line end end
  return table.concat(filtered,';')
end

-- Launch without a shell or inherited stdio pipe. The viewer uses this fast
-- boundary; helper work must never wait for a PowerShell/popen pipe to close.
local launch_defined=false
function M.spawn_native(executable,argv,cwd)
  local ffi=assert(process_api(),'Native launch requires LOVE FFI')
  if not launch_defined then ffi.cdef[[
    typedef struct { unsigned long cb; wchar_t *reserved; wchar_t *desktop; wchar_t *title;
      unsigned long x,y,xsize,ysize,xchars,ychars,fill,flags;
      unsigned short show,reserved_size; unsigned char *reserved_bytes;
      void *input,*output,*error; } UML_STARTUPINFO;
    typedef struct { void *process,*thread; unsigned long pid,tid; } UML_PROCESSINFO;
    int CreateProcessW(const wchar_t*,wchar_t*,void*,void*,int,unsigned long,void*,const wchar_t*,UML_STARTUPINFO*,UML_PROCESSINFO*);
    int MultiByteToWideChar(unsigned int,unsigned long,const char*,int,wchar_t*,int);
  ]]; launch_defined=true end
  local function wide(text)
    local n=ffi.C.MultiByteToWideChar(65001,0,text,-1,nil,0)
    local buffer=ffi.new('wchar_t[?]',n)
    assert(ffi.C.MultiByteToWideChar(65001,0,text,-1,buffer,n)>0)
    return buffer
  end
  local words={M.win_quote(executable)}
  for _,word in ipairs(argv or {}) do words[#words+1]=M.win_quote(word) end
  local info=ffi.new('UML_STARTUPINFO[1]'); info[0].cb=ffi.sizeof('UML_STARTUPINFO')
  local child=ffi.new('UML_PROCESSINFO[1]')
  local ok=ffi.C.CreateProcessW(wide(executable),wide(table.concat(words,' ')),nil,nil,0,
    0x08000000,nil,cwd and wide(cwd) or nil,info,child)
  if ok==0 then error('Native launch failed (Windows error '..tonumber(ffi.C.GetLastError())..')',0) end
  local creation=ffi.new('UML_FILETIME[1]')
  local scratch=ffi.new('UML_FILETIME[1]')
  local observed=ffi.C.GetProcessTimes(child[0].process,creation,scratch,scratch,scratch)
  local pid=tonumber(child[0].pid)
  ffi.C.CloseHandle(child[0].thread); ffi.C.CloseHandle(child[0].process)
  assert(observed~=0,'Native child creation identity could not be read')
  local Identity=require('uml.domain.identity')
  local ticks=Identity.filetime(tonumber(creation[0].dwHighDateTime),tonumber(creation[0].dwLowDateTime))
  return {pid=pid,start=Identity.dotnet_ticks(ticks)}
end

-- The launched process can outlive the PowerShell launcher and may inherit a
-- stdio pipe indefinitely. os.execute plus a result file never waits on that
-- pipe; the launcher's own output is not captured.
local function launch_result_path(cwd,log_path)
  local base=log_path
  if not base then
    local root=os.getenv('TEMP') or os.getenv('TMP') or cwd or '.'
    base=root:gsub('[\\/]$','')..'/uml-spawn'
  end
  local unique=((tostring({}):match('0x(%x+)')) or tostring(os.time()))
    ..'-'..tostring(math.floor(os.clock()*1000000))
  return base..'.launch-'..unique..'.json'
end

-- Detached project-owned process. Returns its exact creation identity.
function M.spawn_detached(executable,argv,cwd,log_path,error_log)
  local result_path=launch_result_path(cwd,log_path)
  local script=start_script(executable,argv,cwd,log_path,error_log,true,result_path)
  os.execute('pwsh.exe -NoProfile -NonInteractive -EncodedCommand '..M.encoded(script))
  local file=io.open(result_path,'rb')
  if not file then error('Detached process could not be launched: no launch result',0) end
  local text=file:read('*a'); file:close(); os.remove(result_path)
  local ok,value=pcall(JSON.decode,text)
  if not ok or type(value)~='table' or type(value.pid)~='number' or type(value.start)~='string' then
    error('Detached process could not be launched: '..text,0)
  end
  return {pid=value.pid,start=value.start,name=value.name}
end

-- Start a child in a detached waiter that inherits this process's stdio (the
-- Herdr pane). The waiter records the exact child identity before waiting and
-- the actual exit code afterward, so the runner can journal :started before
-- the child dies without losing interactive stdio.
function M.start_attached(argv,cwd,env_remove,started_path,exit_path)
  local inner={
    "$ErrorActionPreference='Stop'",
    '$psi=[Diagnostics.ProcessStartInfo]::new('..ps_word(argv[1])..')',
    '$psi.UseShellExecute=$false',
    'foreach($a in @('..ps_argv({unpack(argv,2)})..')){[void]$psi.ArgumentList.Add([string]$a)}',
    (cwd and cwd~='' and ('$psi.WorkingDirectory='..ps_word(cwd))) or '$null',
    'foreach($n in @('..ps_argv(env_remove or {})..')){[void]$psi.Environment.Remove([string]$n)}',
    '$child=[Diagnostics.Process]::Start($psi)',
    '@{pid=$child.Id;start=$child.StartTime.ToUniversalTime().Ticks.ToString();name=$child.ProcessName}|ConvertTo-Json -Compress'
      ..'|Set-Content -LiteralPath '..ps_word(started_path)..' -Encoding utf8',
    '$child.WaitForExit()',
    '@{exit=$child.ExitCode}|ConvertTo-Json -Compress|Set-Content -LiteralPath '..ps_word(exit_path)..' -Encoding utf8',
  }
  local inner_filtered={}
  for _,line in ipairs(inner) do if line~='$null' then inner_filtered[#inner_filtered+1]=line end end
  local waiter_path=started_path..'.waiter.ps1'
  local waiter_file=assert(io.open(waiter_path,'wb'))
  waiter_file:write(table.concat(inner_filtered,';')); waiter_file:close()
  -- Keep the synchronous launcher below the Windows command-line limit; the
  -- waiter script lives in the project journal directory.
  local launcher="$ErrorActionPreference='Stop';"
    ..'$p=Start-Process -FilePath "pwsh.exe" -NoNewWindow -PassThru -ArgumentList @("-NoProfile","-NonInteractive","-File",'
    ..ps_word(M.win_quote(waiter_path))..');'
    ..'@{pid=$p.Id;start=$p.StartTime.ToUniversalTime().Ticks.ToString();name=$p.ProcessName}|ConvertTo-Json -Compress'
    ..'|Set-Content -LiteralPath '..ps_word(started_path..'.waiter.json')..' -Encoding utf8'
  -- os.execute (not io.popen) keeps the waiter's stdio on the pane and avoids
  -- holding a pipe open for the waiter's entire lifetime.
  os.execute('pwsh.exe -NoProfile -NonInteractive -EncodedCommand '..M.encoded(launcher))
  local file=io.open(started_path..'.waiter.json','rb')
  if not file then error('Runner waiter could not be launched',0) end
  local text=file:read('*a'); file:close()
  local ok,value=pcall(JSON.decode,text)
  if not ok or type(value)~='table' or type(value.pid)~='number' then
    error('Runner waiter could not be launched: '..text,0)
  end
  return value
end

local function read_json(path)
  local file=io.open(path,'rb'); if not file then return nil end
  local text=file:read('*a'); file:close()
  local ok,value=pcall(JSON.decode,text); return ok and value or nil
end

-- Wait for the waiter (and therefore the child) to exit. File polling is cheap;
-- a periodic native liveness check detects a waiter that died without writing
-- a final record (unknown cause).
function M.await_exit(waiter,exit_path,options)
  options=options or {}
  local sleep=options.sleep or function(ms) os.execute('ping -n 1 -w '..ms..' 127.0.0.1 >NUL') end
  local checks=0
  while true do
    local ended=read_json(exit_path)
    if ended then
      if type(ended.exit)=='number' then return ended.exit end
      return nil
    end
    sleep(100)
    checks=checks+1
    if checks%50==0 and M.owner_native(waiter.pid)==nil then return nil end
  end
end
return M
