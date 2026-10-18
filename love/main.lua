local log=require('uml.adapters.log')
local game_args=love.arg.parseGameArguments(arg or {})
local function canonical_path(path)
  local ok,result=pcall(function() return require('uml.adapters.windows_path').canonical(path) end)
  if ok then return result end
  return (path or ''):gsub('\\','/')
end
local function love_paths()
  local executable=love.filesystem.getExecutablePath()
  local game=love.filesystem.getSource()
  return executable,game
end
local headless=game_args[1]=='ir'
if headless then
  function love.run()
    local args={}; for i=2,#game_args do args[#args+1]=game_args[i] end
    local ok,result=pcall(function() return require('uml.main.ir_generator').run(args) end)
    if not ok then io.stderr:write('IR generation failed: ',tostring(result),'\n') end
    local code=ok and result or 1
    return function() return code end
  end
  return
end
if game_args[1]=='companion-runner' then
  function love.run()
    local Clock=require('uml.adapters.clock')
    local Recovery=require('uml.adapters.recovery')
    local ok,result=pcall(function()
      local identity=Recovery.read_state(game_args[2])
      return Recovery.run_child(identity,{sleep=Clock.sleep,clock=Clock.monotonic_ms})
    end)
    local code
    if ok then code=tonumber(result) or 1
    else io.stderr:write('UML recovery runner failed: ',tostring(result),'\n'); code=1 end
    return function() return code end
  end
  return
end
if game_args[1]=='companion-controller' then
  function love.run()
    local Clock=require('uml.adapters.clock')
    local Recovery=require('uml.adapters.recovery')
    local Herdr=require('uml.adapters.herdr')
    local Preflight=require('uml.adapters.companion_preflight')
    local executable,game=love_paths()
    local ok,message=pcall(function()
      local identity=Recovery.read_state(game_args[2])
      local selected
      local function select_once()
        if not selected then
          selected=Preflight.select(identity.cwd,{'--backend','herdr'},
            {os='Windows',canonical=canonical_path}).selected
        end
        return selected
      end
      local function launch(submission)
        local backend=Herdr.new(submission,{selected=select_once(),canonical=canonical_path,
          runner={executable=executable,argv={executable,game,'companion-runner'}}})
        backend.relaunch(submission)
      end
      Recovery.controller_loop(identity,{launch=launch,sleep=Clock.sleep,clock=Clock.monotonic_ms})
    end)
    if not ok then io.stderr:write('UML recovery controller failed: ',tostring(message),'\n') end
    return function() return ok and 0 or 1 end
  end
  return
end
if game_args[1]=='companion-smoke' then
  -- Isolated native acceptance harness: real Herdr server, workspace, pane,
  -- runner, controller and cleanup. Never runs against the examined project.
  function love.run()
    local Clock=require('uml.adapters.clock')
    local E=require('uml.edn')
    local Recovery=require('uml.adapters.recovery')
    local Preflight=require('uml.adapters.companion_preflight')
    local Herdr=require('uml.adapters.herdr')
    local Ownership=require('uml.adapters.ownership')
    local Lifecycle=require('uml.adapters.companion_lifecycle')
    local root,fake=game_args[2],game_args[3]
    local function say(text) io.stdout:write('SMOKE STEP ',text,'\n'); io.stdout:flush() end
    local ok,message=pcall(function()
      say('paths')
      local executable,game=love_paths()
      say('preflight')
      local selected=Preflight.select(root,{'--backend','herdr'},
        {os='Windows',canonical=canonical_path}).selected
      say('backend')
      local backend=Herdr.new(nil,{selected=selected,canonical=canonical_path,
        runner={executable=executable,argv={executable,game,'companion-runner'}},
        recovery={controller={executable=executable,argv={game,'companion-controller'}}}})
      say('session')
      local session=Lifecycle.session_name(root,Recovery.new_token())
      say('create')
      local owned=backend.create(session,canonical_path(root),{fake,'-n','1','127.0.0.1'})
      say('record')
      local record=Ownership.read(root)
      assert(record~=nil,'ownership record missing')
      assert(owned.session==session,'session identity changed')
      say('cleanup')
      local released=backend.cleanup(session)
      assert(released and released.released==true,'cleanup was not confirmed')
      Ownership.write(root,E.map())
      io.stdout:write('SMOKE PASS session='..session..'\n')
    end)
    if not ok then io.stderr:write('SMOKE FAIL: ',tostring(message),'\n') end
    return function() return ok and 0 or 1 end
  end
  return
end
if game_args[1]=='companion-preflight' then
  -- Read-only backend selection. Never loads Quil/cimgui and never starts a JVM.
  function love.run()
    local root=game_args[2]
    local args={}; for i=3,#game_args do args[#args+1]=game_args[i] end
    local ok,result=pcall(function()
      return require('uml.adapters.companion_preflight').select(root,args,{os='Windows'})
    end)
    local code
    if ok then
      io.stdout:write(result.backend,'\n')
      io.stderr:write('uml: selected Herdr '..result.selected.release..' at '..result.selected.path..'\n')
      code=0
    else
      io.stderr:write('uml: ',tostring(result),'\n')
      code=1
    end
    return function() return code end
  end
  return
end
function love.errorhandler(message)
  log.exception(message,'uncaught on viewer')
  io.stderr:write('UML viewer failed: ',tostring(message),'\n')
  return function() return 1 end
end
local viewer_args={}
for i=(game_args[1]=='viewer' and 2 or 1),#game_args do viewer_args[#viewer_args+1]=game_args[i] end
require('uml.adapters.viewer').run(viewer_args,{source=require('uml.main.source')})
