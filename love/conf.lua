function love.conf(t)
  t.identity='uml-viewer'
  t.version='11.5'
  t.console=true
  local args=love.arg.parseGameArguments(arg or {})
  if args[1]=='ir' then
    t.window=false
    for _,module in ipairs{'window','graphics','audio','joystick','keyboard','mouse','physics','sound','touch','video'} do t.modules[module]=false end
  end
end
