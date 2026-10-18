-- Only the explicit native harness loads this fixture, never run_busted.lua.
-- Permit bounded fixture commands and identities, not arbitrary subprocesses.
local P=require('uml.adapters.process')
local popen=io.popen
local active=false
local spawn_active=false
local execute=os.execute
local file_active=false
os.execute=function(command)
  local allowed=command:match('^pwsh %-NoProfile %-NonInteractive %-EncodedCommand [%w+/=]+$')
    or command:match('^pwsh%.exe %-NoProfile %-NonInteractive %-EncodedCommand [%w+/=]+$')
  assert((spawn_active or file_active) and allowed,'Native fixture forbids os.execute')
  return execute(command)
end
local Atomic=require('uml.adapters.atomic_file')
local write=Atomic.write
Atomic.write=function(path,bytes,replace)
  local temp=os.getenv('TEMP') or os.getenv('TMP')
  assert(temp and path:lower():sub(1,#temp)==temp:lower(),'Native file fixture requires temporary path')
  file_active=true
  local ok,err=pcall(write,path,bytes,replace)
  file_active=false
  if not ok then error(err,0) end
end
io.popen=function(...)
  assert(active,'Native fixture forbids direct io.popen')
  return popen(...)
end
local function scoped(fn,...)
  active=true
  local result={pcall(fn,...)}
  active=false
  if not result[1] then error(result[2],0) end
  return result[2]
end
local owned={}
local run,spawn,owner,native=P.run,P.spawn_detached,P.owner,P.owner_native
local commands={
  ["[Console]::Out.Write('out value'); [Console]::Error.Write('err value'); exit 23"]=true,
  ['Start-Sleep -Seconds 10']=true,
  ["[Console]::Write('U=' + [string]$env:UML_PROCESS_TEST_VALUE)"]=true,
}
P.run=function(argv,timeout,env)
  assert(argv[1]=='pwsh.exe' and argv[2]=='-NoProfile' and argv[3]=='-NonInteractive' and argv[4]=='-Command')
  assert(timeout and timeout<=15000,'Native fixture requires bounded timeout')
  local pid=argv[5]:match('^%[Diagnostics%.Process%]::GetProcessById%((%d+)%)%.WaitForExit%(%)$')
  assert(commands[argv[5]] or (pid and owned[tonumber(pid)]),'Native fixture forbids command')
  return scoped(run,argv,timeout,env)
end
P.spawn_detached=function(exe,argv,...)
  assert(exe=='pwsh.exe' and argv[1]=='-NoProfile' and argv[2]=='-NonInteractive' and argv[3]=='-Command')
  assert(argv[4]=='Start-Sleep -Seconds 2' or argv[4]=='Start-Sleep -Seconds 5'
    or argv[4]:match('^%$p=Start%-Process %-FilePath .-ping%.exe.-Start%-Sleep %-Seconds 3$'),'Native fixture forbids detached command')
  spawn_active=true
  local ok,value=pcall(scoped,spawn,exe,argv,...)
  spawn_active=false
  if not ok then error(value,0) end
  owned[value.pid]=true; return value
end
P.owner=function(pid) assert(owned[pid],'Native fixture requires owned pid'); return scoped(owner,pid) end
-- The terminated ping pid comes from the fixture file; it is read-only and must
-- be absent. Native identity does not launch a subprocess under LuaJIT.
P.owner_native=function(pid) owned[pid]=true; return scoped(native,pid) end
return true
