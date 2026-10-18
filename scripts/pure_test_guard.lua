-- Loaded before Busted discovers specs. No environment/native-mode opt-out:
-- mutation runs this same bootstrap and cannot mutate the guard with src files.
local function blocked(name)
  return function() error('Pure tests forbid native process access: '..name,0) end
end
os.execute=blocked('os.execute')
io.popen=blocked('io.popen')
local Process=require('uml.adapters.process')
for _,name in ipairs{'run','spawn_detached','pwsh','start_attached','owner','owner_native','current_pid','await_exit'} do
  Process[name]=blocked('Process.'..name)
end
-- Temporary-file fixture for application persistence tests. The native atomic
-- replacement contract is exercised separately, without enabling process access
-- in this suite. Failure callbacks still reach the real write implementation.
local Atomic=require('uml.adapters.atomic_file')
local write=Atomic.write
local function fixture_replace(source,target)
  local reserved=os.tmpname()
  local backup=target..'.'..reserved:gsub('.*[/\\\\]','')..'.fixture-backup'
  os.remove(reserved)
  local had=io.open(target,'rb')
  if had then
    had:close()
    local moved,err=os.rename(target,backup)
    if not moved then return nil,err end
  end
  local ok,err=os.rename(source,target)
  if not ok and had then assert(os.rename(backup,target)) end
  if ok and had then assert(os.remove(backup)) end
  return ok,err
end
Atomic.write=function(path,bytes,replace)
  return write(path,bytes,replace or fixture_replace)
end
Atomic.ensure_parent=function(path)
  local lfs=require('lfs')
  local function mkdir(p)
    if not p or p=='' or lfs.attributes(p,'mode')=='directory' then return end
    mkdir(p:match('^(.*)[/\\\\]')); assert(lfs.mkdir(p))
  end
  mkdir(path:match('^(.*)[/\\\\]'))
end
return true
