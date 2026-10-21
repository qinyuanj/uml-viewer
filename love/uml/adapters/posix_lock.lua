-- OS-owned file locks. flock() locks belong to the open file description and
-- are released when the holding process exits, which is exactly the singleton
-- and gate semantics the lifecycle needs: the lock never relies on stale
-- lock-file contents, and no manual stale-lock cleanup is possible or needed.
-- FFI is loaded lazily: the pure suite runs under Lua 5.4, where no FFI
-- exists. Locks are only taken by real lifecycle processes.
local M={}
local ffi
local function api()
  if ffi then return ffi end
  local ok,value=pcall(require,'ffi')
  assert(ok,'OS file locks require LuaJIT FFI')
  value.cdef[[
int open(const char *pathname, int flags, unsigned int mode);
int flock(int fd, int operation);
int close(int fd);
]]
  ffi=value
  return ffi
end
local LOCK_EX=2
local LOCK_NB=4
local O_RDWR=2
local O_CREAT=0x40
-- Close-on-exec: a spawned Herdr server or controller must not inherit the
-- descriptor, or the lock outlives its holder and blocks the next owner.
local O_CLOEXEC=0x80000
function M.try_acquire(path)
  api()
  local fd=ffi.C.open(path,bit.bor(O_RDWR,O_CREAT,O_CLOEXEC),420)
  if fd<0 then error('Cannot open lock file '..path,0) end
  if ffi.C.flock(fd,bit.bor(LOCK_EX,LOCK_NB))~=0 then
    ffi.C.close(fd)
    return nil
  end
  return {fd=fd}
end
function M.acquire(path,options)
  options=options or {}
  local sleep=options.sleep or function(ms) local deadline=os.clock()+ms/1000; while os.clock()<deadline do end end
  while true do
    local handle=M.try_acquire(path)
    if handle then return handle end
    sleep(25)
  end
end
function M.release(handle)
  if not handle then return end
  api()
  ffi.C.close(handle.fd)
end
return M
