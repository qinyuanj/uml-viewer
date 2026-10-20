-- POSIX process boundary. argv is passed literally to posix_spawn: no shell is
-- ever involved, so spaces and metacharacters cannot be reinterpreted.
-- Output/status are returned as data, like the Windows boundary.
-- Creation identity is the kernel's starttime jiffies sampled from /proc; every
-- process observing the same PID reads the identical decimal text, so PID reuse
-- is detected by an exact string comparison.
--
-- FFI is loaded lazily: the pure test suite runs under Lua 5.4, where no FFI
-- exists. Only an actual process call requires LuaJIT.
local M={}
local ffi
local CLOCK_MONOTONIC=1
local WNOHANG=1
local SIGKILL=9
local O_WRONLY=1
local O_CREAT=0x40
local O_TRUNC=0x200
local POSIX_SPAWN_SETSID=0x80
local FILE_ACTIONS_BYTES=1024
local ATTR_BYTES=1024

local function api()
  if ffi then return ffi end
  local ok,value=pcall(require,'ffi')
  assert(ok,'The POSIX process boundary requires LuaJIT FFI')
  value.cdef[[
typedef int pid_t;
int posix_spawn_file_actions_init(void *fa);
int posix_spawn_file_actions_addopen(void *fa, int fd, const char *path, int oflag, unsigned int mode);
int posix_spawn_file_actions_addchdir_np(void *fa, const char *path);
int posix_spawn_file_actions_destroy(void *fa);
int posix_spawnattr_init(void *attr);
int posix_spawnattr_setflags(void *attr, short flags);
int posix_spawn(pid_t *pid, const char *path, const void *fa, const void *attr, char *const argv[], char *const envp[]);
int waitpid(pid_t pid, int *status, int options);
int kill(pid_t pid, int sig);
int getpid(void);
int nanosleep(const void *req, void *rem);
typedef struct { long tv_sec; long tv_nsec; } uml_posix_timespec;
int clock_gettime(int clockid, uml_posix_timespec *tp);
]]
  ffi=value
  return ffi
end

-- Opaque libc structs: we never inspect them, only let glibc own the storage.
local function file_actions()
  api()
  local actions=ffi.new('char[?]',FILE_ACTIONS_BYTES)
  assert(ffi.C.posix_spawn_file_actions_init(actions)==0,'posix_spawn_file_actions_init failed')
  return actions
end
local function attributes()
  api()
  local attr=ffi.new('char[?]',ATTR_BYTES)
  assert(ffi.C.posix_spawnattr_init(attr)==0,'posix_spawnattr_init failed')
  return attr
end
local function carray(list)
  api()
  local out=ffi.new('char *[?]',#list+1)
  for index,value in ipairs(list) do out[index-1]=ffi.cast('char *',tostring(value)) end
  out[#list]=nil
  return out
end
local function read_file(path)
  local file=io.open(path,'rb'); if not file then return nil end
  local text=file:read('*a'); file:close(); return text
end

function M.windows() return false end
function M.current_pid() api(); return tonumber(ffi.C.getpid()) end

function M.monotonic_ms()
  api()
  local ts=ffi.new('uml_posix_timespec')
  if ffi.C.clock_gettime(CLOCK_MONOTONIC,ts)~=0 then return nil end
  return tonumber(ts.tv_sec)*1000+math.floor(tonumber(ts.tv_nsec)/1000000)
end
function M.sleep(ms)
  api()
  local request=ffi.new('uml_posix_timespec')
  request.tv_sec=math.floor(ms/1000)
  request.tv_nsec=(ms%1000)*1000000
  ffi.C.nanosleep(request,nil)
end

-- Environment snapshot without the removed names; kept as literal NAME=value.
function M.environment(remove)
  local excluded={}
  for _,name in ipairs(remove or {}) do excluded[name]=true end
  local raw=(read_file('/proc/self/environ') or ''):gsub('%z','\n')
  local out={}
  for entry in raw:gmatch('[^\n]+') do
    local name=entry:match('^([^=]+)=')
    if name and not excluded[name] then out[#out+1]=entry end
  end
  return carray(out)
end

-- /proc/<pid>/stat: field 2 is the command and may contain spaces and parens,
-- so parse from the last ')' and count the fields after it. A zombie is not a
-- running process: an exited child keeps its /proc entry until its parent
-- reaps it, and a detached controller has no waiter by design.
local function stat_start(pid)
  local text=read_file('/proc/'..tostring(pid)..'/stat')
  if not text then return nil end
  local rest=text:match('^.*%)%s*(.*)$')
  if not rest then return nil end
  local fields={}
  for value in rest:gmatch('%S+') do fields[#fields+1]=value end
  -- fields[1] is field 3 (state); starttime is field 22, so index 20 here.
  if fields[1]=='Z' or fields[1]=='X' then return nil end
  return fields[20]
end

function M.identity(pid)
  if type(pid)=='string' and pid:match('^[1-9]%d*$') then pid=tonumber(pid) end
  if type(pid)~='number' or pid~=math.floor(pid) or pid<1 then
    error('Invalid owned process PID: '..tostring(pid),0)
  end
  local start=stat_start(pid)
  if start==nil then return nil end
  local name=read_file('/proc/'..pid..'/comm')
  name=name and name:gsub('%s+$','') or ''
  return {pid=pid,start=start,name=name}
end

function M.owner(pid) return M.identity(pid) end
function M.owner_native(pid)
  local value=M.identity(pid); if not value then return nil end
  return {pid=value.pid,start=value.start}
end

local counter=0
local function temp_path(tag)
  api()
  local base=os.getenv('TMPDIR') or '/tmp'
  counter=counter+1
  return ('%s/uml-%s-%d-%d-%d'):format(base,tag,tonumber(ffi.C.getpid()),counter,math.floor(os.clock()*1000000))
end

local function wait_status(status)
  local value=status[0]
  local signal=bit.band(value,0x7f)
  if signal~=0 then return 128+signal end
  return bit.band(bit.rshift(value,8),0xff)
end

-- Bounded synchronous command with captured stdio. A timeout kills only the
-- child this call spawned.
function M.run(argv,timeout_ms,env_remove)
  timeout_ms=timeout_ms or 10000
  assert(type(argv)=='table' and argv[1],'run requires a nonempty argv')
  local out_path,err_path=temp_path('out'),temp_path('err')
  local actions=file_actions()
  local flags=O_WRONLY+O_CREAT+O_TRUNC
  ffi.C.posix_spawn_file_actions_addopen(actions,1,out_path,flags,420)
  ffi.C.posix_spawn_file_actions_addopen(actions,2,err_path,flags,420)
  local pid=ffi.new('pid_t[1]')
  local spawned=ffi.C.posix_spawn(pid,argv[1],actions,nil,carray(argv),M.environment(env_remove))
  ffi.C.posix_spawn_file_actions_destroy(actions)
  if spawned~=0 then
    os.remove(out_path); os.remove(err_path)
    return {exit=nil,out='',err='process boundary failed (posix_spawn error '..spawned..')',timeout=false}
  end
  local status=ffi.new('int[1]')
  local deadline=M.monotonic_ms()+timeout_ms
  local timed_out=false
  while true do
    local reaped=ffi.C.waitpid(pid[0],status,WNOHANG)
    if reaped==pid[0] then break end
    if M.monotonic_ms()>=deadline then
      ffi.C.kill(pid[0],SIGKILL)
      ffi.C.waitpid(pid[0],status,0)
      timed_out=true; break
    end
    M.sleep(5)
  end
  local out=read_file(out_path) or ''
  local err=read_file(err_path) or ''
  os.remove(out_path); os.remove(err_path)
  local exit_code=nil
  if not timed_out then exit_code=wait_status(status) end
  local error_text=err:gsub('%s+$','')
  if timed_out then error_text='process timed out' end
  return {exit=exit_code,out=out:gsub('%s+$',''),err=error_text,timeout=timed_out}
end

-- Detached project-owned process in its own session; stdio goes to the given
-- logs. Returns its exact creation identity.
function M.spawn_detached(executable,argv,cwd,log_path,error_log)
  local actions=file_actions()
  if cwd and cwd~='' then
    local changed=ffi.C.posix_spawn_file_actions_addchdir_np(actions,cwd)
    if changed~=0 then
      ffi.C.posix_spawn_file_actions_destroy(actions)
      error('Detached process could not enter '..cwd..' (error '..changed..')',0)
    end
  end
  local flags=O_WRONLY+O_CREAT+O_TRUNC
  ffi.C.posix_spawn_file_actions_addopen(actions,0,'/dev/null',0,0)
  ffi.C.posix_spawn_file_actions_addopen(actions,1,log_path or '/dev/null',flags,420)
  ffi.C.posix_spawn_file_actions_addopen(actions,2,error_log or log_path or '/dev/null',flags,420)
  local attr=attributes()
  assert(ffi.C.posix_spawnattr_setflags(attr,POSIX_SPAWN_SETSID)==0,'posix_spawnattr_setflags failed')
  local pid=ffi.new('pid_t[1]')
  local full={executable}
  for _,value in ipairs(argv or {}) do full[#full+1]=value end
  local spawned=ffi.C.posix_spawn(pid,executable,actions,attr,carray(full),M.environment(nil))
  ffi.C.posix_spawn_file_actions_destroy(actions)
  if spawned~=0 then error('Detached process could not be launched (posix_spawn error '..spawned..')',0) end
  local observed=M.identity(pid[0])
  if not observed then error('Detached process exited before its identity was read',0) end
  return observed
end

-- Child inheriting this process's stdio (the Herdr pane).
function M.spawn_native(executable,argv,cwd)
  local actions=file_actions()
  if cwd and cwd~='' then ffi.C.posix_spawn_file_actions_addchdir_np(actions,cwd) end
  local pid=ffi.new('pid_t[1]')
  local full={executable}
  for _,value in ipairs(argv or {}) do full[#full+1]=value end
  local spawned=ffi.C.posix_spawn(pid,executable,actions,nil,carray(full),M.environment(nil))
  ffi.C.posix_spawn_file_actions_destroy(actions)
  if spawned~=0 then error('Native launch failed (posix_spawn error '..spawned..')',0) end
  local observed=M.identity(pid[0])
  if not observed then error('Native child exited before its identity was read',0) end
  return observed
end

-- Child in this process's own stdio (the Herdr pane). Records its exact
-- identity before waiting, then publishes the actual exit code.
function M.start_attached(argv,cwd,env_remove,started_path,exit_path)
  local actions=file_actions()
  if cwd and cwd~='' then ffi.C.posix_spawn_file_actions_addchdir_np(actions,cwd) end
  local pid=ffi.new('pid_t[1]')
  local spawned=ffi.C.posix_spawn(pid,argv[1],actions,nil,carray(argv),M.environment(env_remove))
  ffi.C.posix_spawn_file_actions_destroy(actions)
  if spawned~=0 then error('Runner child could not be launched (posix_spawn error '..spawned..')',0) end
  local observed=M.identity(pid[0])
  if not observed then error('Runner child exited before its identity was read',0) end
  local file=assert(io.open(started_path,'wb'))
  file:write(('{"pid":%d,"start":"%s","name":"%s"}'):format(observed.pid,observed.start,observed.name))
  file:close()
  return {pid=observed.pid,start=observed.start,name=observed.name,exit_path=exit_path}
end

-- Block until the attached child this process spawned exits, then publish the
-- exit code. A child that was not reapable is reported as an unknown exit,
-- never as a verified abnormal one.
function M.await_exit(waiter,exit_path,options)
  options=options or {}
  api()
  local status=ffi.new('int[1]')
  local reaped=ffi.C.waitpid(waiter.pid,status,0)
  local exit_code=nil
  if reaped==waiter.pid then exit_code=wait_status(status) end
  local file=io.open(exit_path or waiter.exit_path,'wb')
  if file then
    if exit_code then file:write(('{"exit":%d}'):format(exit_code)) else file:write('{"exit":null}') end
    file:close()
  end
  return exit_code
end
return M
