-- Pure Herdr ownership rules: pane shells, observed identity and cleanup
-- checkpoints. The adapter owns every external command.
local Text=require('uml.domain.text')
local M={}

function M.fail(message) error(message,0) end

function M.copy(value)
  if type(value)~='table' then return value end
  local out={}; for key,item in pairs(value) do out[key]=M.copy(item) end; return out
end

function M.same(a,b)
  if type(a)~='table' or type(b)~='table' then return a==b end
  for key,item in pairs(a) do if not M.same(item,b[key]) then return false end end
  for key in pairs(b) do if a[key]==nil then return false end end
  return true
end

function M.session_valid(session)
  return type(session)=='string' and session:match('^uml%-viewer%-[%w_%-]+$')~=nil
end

function M.shell_word(text) return Text.shell_word(text) end

function M.pane_command(command,shell_name)
  local name=tostring(shell_name or ''):match('[^/\\]+$') or ''
  name=name:lower():gsub('^%-','')
  local out={}
  if name=='sh' or name=='bash' or name=='zsh' or name=='dash' then
    for _,value in ipairs(command) do out[#out+1]=Text.shell_word(value) end
    return table.concat(out,' ')
  end
  M.fail('Unsupported Herdr pane shell; command was not sent')
end

-- Ownership is built on the identity the backend was constructed with, so
-- lifecycle fields such as the --resume recovery command survive the commit.
function M.ownership_base(identity,session,cwd,command)
  local base=M.copy(identity or {})
  base.backend='herdr'; base.owner_role='companion'; base.session=session
  base.cwd=cwd; base.command=M.copy(command); base.provisional=true
  return base
end

function M.committed(record)
  local owned=M.copy(record); owned.provisional=nil; return owned
end

local FIELDS={'socket','workspace','pane','terminal_id','shell_pid','owner_start','server_pid','server_start','cwd','command'}
function M.complete(identity)
  if not M.session_valid(identity and identity.session) then
    M.fail('Herdr requires an isolated uml-viewer named session')
  end
  if not identity or identity.backend~='herdr' or identity.owner_role~='companion' then
    M.fail('Incomplete Herdr ownership')
  end
  for _,field in ipairs(FIELDS) do
    if identity[field]==nil then M.fail('Incomplete Herdr ownership') end
  end
  return identity
end

function M.observe(expected,status,snapshot,info,canonical)
  M.complete(expected)
  if expected.session~=status.session or expected.socket~=status.socket then
    M.fail('Herdr socket changed')
  end
  if type(snapshot.panes)~='table' or type(snapshot.workspaces)~='table'
      or snapshot.version==nil or snapshot.protocol~=22 then
    M.fail('Invalid Herdr snapshot')
  end
  local matches={}
  for _,pane in ipairs(snapshot.panes) do
    if pane.pane_id==expected.pane then matches[#matches+1]=pane end
  end
  local observed=matches[1]
  local workspace_found=false
  for _,workspace in ipairs(snapshot.workspaces) do
    if workspace.workspace_id==expected.workspace then workspace_found=true end
  end
  local canonical_ok
  if type(observed)=='table' and type(observed.cwd)=='string' then
    canonical_ok=canonical(expected.cwd)==canonical(observed.cwd)
  end
  if #matches~=1 or not workspace_found or type(observed)~='table'
      or observed.workspace_id~=expected.workspace
      or observed.terminal_id~=expected.terminal_id
      or type(info)~='table' or info.pane_id~=expected.pane
      or info.shell_pid~=expected.shell_pid or not canonical_ok then
    M.fail('Herdr pane identity changed')
  end
  return expected
end

function M.foreground_allowed(info,shell_pid)
  local foreground=info and info.foreground_processes
  if type(foreground)~='table' then
    M.fail('Recovery foreground is unknown or occupied; refusing replacement')
  end
  for _,entry in ipairs(foreground) do
    if entry.pid~=shell_pid then
      M.fail('Recovery foreground is unknown or occupied; refusing replacement')
    end
  end
  return true
end

function M.ownership_changed(identity,saved)
  local function without_phase(value)
    local out=M.copy(value); out.cleanup_phase=nil; return out
  end
  return not M.same(without_phase(identity),without_phase(saved))
end

local PHASES={['workspace-closing']=true,['workspace-closed']=true,
  ['server-stopping']=true,['server-stopped']=true,['session-deleting']=true}
function M.cleanup_phase_valid(phase)
  return phase==nil or PHASES[phase]==true
end

function M.runner_event(identity,event)
  if type(event)~='table' or identity.runner_token~=event.token then return nil end
  return event
end

function M.wake_arguments(token,acknowledged,keys)
  if type(token)~='string' or token=='' or type(acknowledged)~='function'
      or type(keys)~='table' or #keys~=2 or keys[1]~='-l'
      or type(keys[2])~='string' or not keys[2]:find(token,1,true) then
    M.fail('Herdr wake requires literal text, a unique token and consumption acknowledgement')
  end
  return true
end
return M
