-- Restart session file IO only; snapshot semantics stay in application.session.
local E=require('uml.edn')
local Atomic=require('uml.adapters.atomic_file')
local M={}
local function path(root) return root:gsub('\\','/')..'/.uml-viewer/session.edn' end
function M.read(root)
  local f=io.open(path(root),'rb'); if not f then return nil end
  local text=f:read('*a'); f:close()
  local ok,value=pcall(E.parse,text)
  if ok and E.kind(value)=='map' then return value end
  return nil
end
-- Default diagram when the user names none: the project's own generated
-- diagram first, then the project-named example, then any example. A project
-- without examples/ falls back to diagram.edn; a restart's saved path wins
-- over this default (application.session.path).
function M.default_diagram(project_root)
  local root=(project_root or '.'):gsub('\\','/'):gsub('/$','')
  local function present(path) local f=io.open(path,'rb'); if f then f:close(); return true end; return false end
  local candidates={root..'/diagram.edn'}
  local name=root:match('([^/]+)$')
  if name then candidates[#candidates+1]=root..'/examples/'..name..'.edn' end
  candidates[#candidates+1]=root..'/examples/uml-viewer.edn'
  for _,path in ipairs(candidates) do if present(path) then return path end end
  local listed
  local lfs_ok,lfs=pcall(require,'lfs')
  if lfs_ok and lfs.attributes(root..'/examples','mode')=='directory' then
    for entry in lfs.dir(root..'/examples') do
      if entry:match('%.edn$') then listed=listed or root..'/examples/'..entry end
    end
  end
  return listed or candidates[1]
end
function M.save(root,snapshot)
  local target=path(root); Atomic.ensure_parent(target); Atomic.write(target,E.encode(snapshot)); return snapshot
end
return M
