-- Companion lifecycle orchestration for the LOVE viewer. Prompt text, session
-- naming and ownership checks are pure; all process work stays in adapters.
local E=require('uml.edn')
local C=require('uml.domain.companion')
local H=require('uml.domain.herdr')
local Ownership=require('uml.adapters.ownership')
local Preflight=require('uml.adapters.companion_preflight')
local Recovery=require('uml.adapters.recovery')
local Herdr=require('uml.adapters.herdr')
local Claude=require('uml.adapters.claude')
local Text=require('uml.domain.text')
local Process=require('uml.adapters.process')
local M={}

local function love_executable()
  if type(love)=='table' and love.filesystem and love.filesystem.getExecutablePath then
    return love.filesystem.getExecutablePath()
  end
  return os.getenv('UML_LOVE_EXECUTABLE')
end
local function game_directory()
  if type(love)=='table' and love.filesystem and love.filesystem.getSource then
    return love.filesystem.getSource()
  end
  return '.'
end

-- Herdr derives its API socket from XDG_CONFIG_HOME + session name. Measured
-- against Herdr 0.9.5: the server starts at 101 bytes but workspace creation
-- fails from 102 bytes, and that failure surfaces as an opaque closed-channel
-- error. The generated name therefore shrinks to whatever the config root
-- leaves, instead of assuming a fixed project-name budget.
M.MAX_SOCKET_BYTES=101
M.MAX_NAME_BYTES=64
function M.config_root()
  local root=os.getenv('XDG_CONFIG_HOME')
  if not root or root=='' then root=(os.getenv('HOME') or '')..'/.config' end
  return (root:gsub('/+$',''))
end
function M.session_name(root,uuid,config_root)
  local base=(root:gsub('\\','/'):match('([^/]+)/*$') or 'project'):gsub('[^%w_%-]','-')
  local hash=0
  for index=1,#root do hash=(hash*31+root:byte(index))%4294967296 end
  -- Fixed parts: the grammar prefix, the project hash and the launch suffix.
  -- The project name is the only part that may shrink.
  local fixed=('uml-viewer-%%s-%08x-%s'):format(hash,(uuid or ''):gsub('%-',''):sub(1,8))
  local socket_prefix=(config_root or M.config_root())..'/herdr/sessions/'
  -- %%s is a two-byte placeholder for the name itself.
  local room=math.min(M.MAX_SOCKET_BYTES-#socket_prefix-#'/herdr.sock',M.MAX_NAME_BYTES) - (#fixed-2)
  if room<1 then
    error('Herdr session name cannot fit: '..socket_prefix..fixed:gsub('%%s','<project>')
      ..'/herdr.sock exceeds the '..M.MAX_SOCKET_BYTES..'-byte Unix socket path;'
      ..' set a shorter XDG_CONFIG_HOME',0)
  end
  return fixed:format(base:sub(1,room))
end
function M.load(root,options)
  options=options or {}
  local value=Ownership.read(root)
  if value==nil or (E.kind(value)=='map' and #value==0) then return nil end
  C.recorded_backend(value)
  local plain=Recovery.from_edn(value)
  H.complete(plain)
  local canonical=options.canonical or function(path) return (path or ''):gsub('\\','/') end
  if canonical(plain.cwd)~=canonical(root) then
    error('Companion ownership is missing, ambiguous or belongs to another project; retained',0)
  end
  return plain
end

function M.required(root,options)
  local plain=M.load(root,options)
  if not plain then error('Companion ownership is missing, ambiguous or unreadable; retained',0) end
  return plain
end

local function factory(options,selected,identity)
  if options and options.backend then return options.backend(selected) end
  local executable=love_executable()
  local game=game_directory()
  return Herdr.new(identity,{
    run=options and options.run,owner=options and options.owner,
    canonical=options and options.canonical,selected=selected,
    runner={executable=executable,argv={executable,game,'companion-runner'}},
    recovery={controller={executable=executable,argv={game,'companion-controller'}}},
    progress=options and options.progress,
  })
end

function M.select(root,options)
  local preflight=options and options.preflight or {}
  return Preflight.select(root,{'--backend','herdr'},preflight).selected
end

local function on_path(name)
  for directory in (os.getenv('PATH') or ''):gmatch('[^:]+') do
    local candidate=directory..'/'..name
    local handle=io.open(candidate,'rb')
    if handle then handle:close(); return candidate end
  end
end

-- WSL reaches the host's Windows Terminal through interop; the new tab enters
-- the same distribution and attaches with the selected Herdr's absolute path
-- (a fresh WSL login has no ~/.local/bin on PATH). Elsewhere, or when the
-- terminal does not start, the caller reports the command a person can run.
function M.open_window(session,options)
  options=options or {}
  local env=options.env or os.getenv
  local herdr=options.herdr or 'herdr'
  local attach=Text.shell_word(herdr)..' session attach '..Text.shell_word(session)
  local distro=env('WSL_DISTRO_NAME')
  local terminal=distro and distro~='' and (options.which or on_path)('wt.exe')
  if not terminal then return {terminal='none',session=session,attach=attach} end
  local argv={terminal,'-w','new','new-tab','--title',session,'wsl.exe','-d',distro,'--exec'}
  local config=env('XDG_CONFIG_HOME')
  if config and config~='' then argv[#argv+1]='/usr/bin/env'; argv[#argv+1]='XDG_CONFIG_HOME='..config end
  for _,word in ipairs{herdr,'session','attach',session} do argv[#argv+1]=word end
  local result=(options.run or Process.run)(argv,10000)
  local ok=type(result)=='table' and result.exit==0
  return {terminal=ok and 'windows-terminal' or 'none',session=session,attach=attach}
end

function M.create(root,options)
  options=options or {}
  if M.load(root,options) then
    error('Companion ownership already recorded; use --restart or resolve it before fresh start',0)
  end
  local selected=M.select(root,options)
  local claude=(options.claude or Claude.executable)(options.claude_options)
  local uuid=(options.uuid or Recovery.new_token)()
  local session=M.session_name(root,uuid)
  local command={claude,'--dangerously-skip-permissions','--session-id',uuid,
    '--append-system-prompt',C.standing_rules,C.launch_prompt}
  local recovery_command={claude,'--dangerously-skip-permissions','--resume',uuid,
    '--append-system-prompt',C.standing_rules,C.recovery_prompt}
  local canonical=options.canonical or function(path) return (path or ''):gsub('\\','/') end
  Ownership.write(root,Recovery.to_edn{backend='herdr',owner_role='companion',
    cwd=canonical(root),session=session,command=command,recovery_command=recovery_command,
    claude_session=uuid,provisional=true})
  local B=factory(options,selected,nil)
  local owned=B.create(session,canonical(root),command)
  B.identity=owned
  return owned,B
end
function M.restart(root,options)
  options=options or {}
  local plain=M.load(root,options)
  if not plain then error('Companion ownership is missing; cannot restart',0) end
  local selected=M.select(root,options)
  local B=factory(options,selected,plain)
  B.probe(plain.session)
  B.bind(plain.session,plain.pane)
  Recovery.verify(plain,options.owner)
  return plain,B
end

function M.shutdown(root,options)
  options=options or {}
  local plain=M.load(root,options)
  if not plain then return {released=false} end
  local selected=M.select(root,options)
  local B=factory(options,selected,plain)
  B.cleanup(plain.session)
  Ownership.write(root,E.map())
  return {released=true}
end

return M
