local log=require('uml.adapters.log')
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
