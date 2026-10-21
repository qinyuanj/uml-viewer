-- Read-only companion backend preflight. Selection is pure; this adapter only
-- supplies PATH candidates, executable probing, ownership bytes and commands.
local C=require('uml.domain.companion')
local Ownership=require('uml.adapters.ownership')
local Process=require('uml.adapters.process')
local M={}

function M.default_probe(path)
  local file=io.open(path,'rb')
  if not file then return nil end
  file:close()
  -- A readable file is enough to offer as a candidate; the version and schema
  -- probes decide whether it is really Herdr.
  local canonical=path:gsub('\\','/')
  return {executable=true,canonical=canonical}
end

-- The host as LOVE names it (WSL reports Linux); the pure preflight decides
-- which names are delivered. Without LOVE the POSIX entry is the caller.
function M.host_os(system)
  system=system or (love and love.system)
  return system and system.getOS() or 'Linux'
end

function M.select(root,args,options)
  options=options or {}
  local platform=options.os or M.host_os()
  local selection=C.selection_args(args)
  local record=Ownership.read(root)
  C.preflight_checks(record,{root=root,requested=selection.backend,restart=options.restart,
    os=platform,canonical=options.canonical or function(path) return path end})
  local candidates=C.herdr_candidates(platform,options.path or os.getenv('PATH') or '',
    options.probe or M.default_probe)
  local selected=C.select_herdr(candidates,options.run or Process.run)
  return {backend='herdr',args=selection.args,selected=selected}
end
return M
