function love.conf(t)
  t.identity='uml-viewer'
  t.version='11.5'
  t.console=true
  local args=love.arg.parseGameArguments(arg or {})
  -- Helper modes replace love.run without an event loop; a window would hang.
  local headless={['ir']=true,['companion-worker']=true,['companion-runner']=true,
    ['companion-controller']=true,['companion-smoke']=true,['companion-preflight']=true}
  if headless[args[1]] then
    t.window=false
    for _,module in ipairs{'window','graphics','audio','joystick','keyboard','mouse','physics','sound','touch','video'} do t.modules[module]=false end
  end
end
