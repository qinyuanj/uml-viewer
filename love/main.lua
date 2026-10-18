local log=require('uml.adapters.log')
local game_args=love.arg.parseGameArguments(arg or {})
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
function love.errorhandler(message)
  log.exception(message,'uncaught on viewer')
  io.stderr:write('UML viewer failed: ',tostring(message),'\n')
  return function() return 1 end
end
local viewer_args={}
for i=(game_args[1]=='viewer' and 2 or 1),#game_args do viewer_args[#viewer_args+1]=game_args[i] end
require('uml.adapters.viewer').run(viewer_args,{source=require('uml.main.source')})
