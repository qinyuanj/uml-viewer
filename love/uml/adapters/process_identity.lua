-- Native Windows process identity boundary. LOVE 11.5 exposes no process API,
-- so kernel32 supplies the actual PID and full-precision creation FILETIME.
-- The pure identity module owns decimal text; this adapter only samples FFI.
local ffi=require('ffi')
local Identity=require('uml.domain.identity')
local Atomic=require('uml.adapters.atomic_file')
ffi.cdef[[
typedef struct { unsigned long dwLowDateTime; unsigned long dwHighDateTime; } UML_FILETIME;
unsigned long GetCurrentProcessId(void);
void* GetCurrentProcess(void);
int GetProcessTimes(void*,UML_FILETIME*,UML_FILETIME*,UML_FILETIME*,UML_FILETIME*);
]]
local M={}
function M.record(root)
  assert(type(root)=='string' and #root>0,'LOVE process identity needs a project root')
  local creation=ffi.new('UML_FILETIME[1]')
  local exited=ffi.new('UML_FILETIME[1]')
  local kernel=ffi.new('UML_FILETIME[1]')
  local user=ffi.new('UML_FILETIME[1]')
  assert(ffi.C.GetProcessTimes(ffi.C.GetCurrentProcess(),creation,exited,kernel,user)~=0,
    'GetProcessTimes failed for the LOVE viewer')
  local record={
    pid=tonumber(ffi.C.GetCurrentProcessId()),
    started=Identity.filetime(tonumber(creation[0].dwHighDateTime),tonumber(creation[0].dwLowDateTime)),
    runtime='love',
    cwd=root:gsub('\\','/'),
  }
  local target=record.cwd..'/.uml-viewer/viewer-process-identity.json'
  Atomic.ensure_parent(target); Atomic.write(target,Identity.encode(record))
  return record,target
end
return M
