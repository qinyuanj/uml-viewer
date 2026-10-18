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
function love.load()
  love.window.setTitle('UML viewer — Lua skeleton')
end
function love.draw()
  love.graphics.print('UML viewer: Lua skeleton',20,20)
end
local original_errorhandler=love.errorhandler
function love.errorhandler(message)
  log.exception(message,'uncaught on viewer')
  if original_errorhandler then return original_errorhandler(message) end
end
