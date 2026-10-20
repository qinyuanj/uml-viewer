-- LOVE viewer process identity boundary. LOVE 11.5 exposes no process API, so
-- the running PID and its creation identity are read from the platform process
-- boundary and written in one document. The identity text is opaque to the
-- restart wrapper: it compares it exactly, and never interprets it.
local Identity=require('uml.domain.identity')
local Atomic=require('uml.adapters.atomic_file')
local Process=require('uml.adapters.process')
local M={}
function M.record(root)
  assert(type(root)=='string' and #root>0,'LOVE process identity needs a project root')
  local observed=Process.owner_native(Process.current_pid())
  assert(observed~=nil,'Could not read the LOVE viewer creation identity')
  local record={
    pid=observed.pid,
    started=observed.start,
    runtime='love',
    cwd=root:gsub('\\','/'),
  }
  local target=record.cwd..'/.uml-viewer/viewer-process-identity.json'
  Atomic.ensure_parent(target); Atomic.write(target,Identity.encode(record))
  return record,target
end
return M
