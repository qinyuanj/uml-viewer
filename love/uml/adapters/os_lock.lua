-- OS-owned file locks. Windows sharing mode 0 gives the same singleton and
-- gate semantics as the JVM FileChannel contract: the lock dies with the
-- process and never relies on stale lock-file contents.
local M={}
local ffi_api
local function api()
  if ffi_api~=nil then return ffi_api or nil end
  local ok,ffi=pcall(require,'ffi')
  if not ok then ffi_api=false; return nil end
  ffi.cdef[[
    int MultiByteToWideChar(unsigned int,unsigned long,const char*,int,wchar_t*,int);
    void* CreateFileW(const wchar_t*,unsigned long,unsigned long,void*,unsigned long,unsigned long,void*);
    int CloseHandle(void*);
    unsigned long GetLastError(void);
  ]]
  ffi_api=ffi; return ffi
end
local function wide(ffi,path)
  local count=ffi.C.MultiByteToWideChar(65001,0,path,-1,nil,0)
  assert(count>0,'invalid UTF-8 lock path')
  local buffer=ffi.new('wchar_t[?]',count)
  assert(ffi.C.MultiByteToWideChar(65001,0,path,-1,buffer,count)>0)
  return buffer
end
local GENERIC_WRITE=0x40000000
local OPEN_ALWAYS=4
local FILE_ATTRIBUTE_NORMAL=0x80
function M.try_acquire(path)
  local ffi=api()
  if not ffi then error('OS file locks require the LOVE FFI boundary',0) end
  local handle=ffi.C.CreateFileW(wide(ffi,path),GENERIC_WRITE,0,nil,OPEN_ALWAYS,FILE_ATTRIBUTE_NORMAL,nil)
  if handle==ffi.cast('void*',-1) then
    local code=tonumber(ffi.C.GetLastError())
    if code==32 or code==33 or code==5 then return nil end
    error('Cannot open recovery lock '..path..' (Windows error '..code..')',0)
  end
  return handle
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
  local ffi=api()
  if ffi and handle then ffi.C.CloseHandle(handle) end
end
return M
