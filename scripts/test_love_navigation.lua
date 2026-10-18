-- Runs only in the isolated acceptance process via UML_VIEWER_TEST_DRIVER.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local function check(ok,message) assert(ok,message) end
local function dump(label,s)
  local f=assert(io.open(os.getenv('UML_VIEWER_EVIDENCE')..'/navigation-'..label..'.edn','wb'))
  f:write(E.encode(G.map{focus=s.focus,zoom=s.zoom or 1,['cam-x']=s.cam_x or 0,['cam-y']=s.cam_y or 0,selected=s.selected or E.nil_value})); f:close()
end
return function(frame,s)
  if frame==2 then
    check(#s.focus==0,'initial root'); dump('root',s)
    -- Fixed fixture source rectangle begins at x=68,y=100.
    love.mousemoved(78,105); love.mousepressed(78,105,1,false,2)
  elseif frame==3 then
    check(E.equal(s.focus,E.parse('[:source]')),'double click enters source'); dump('entered',s)
    love.keypressed('right')
  elseif frame==4 then
    check(s.cam_x==96,'right arrow pans 96 world pixels'); dump('panned',s)
    -- Control is a system boundary; this process-local input shim avoids
    -- injecting desktop keystrokes into the maintainer viewer.
    local old=love.keyboard.isDown
    love.keyboard.isDown=function(...) if select(1,...)=='lctrl' then return true end; return old(...) end
    love.keypressed('='); love.keyboard.isDown=old
  elseif frame==5 then
    check(s.zoom==1.1,'control equals zooms 10 percent'); check(math.abs(s.cam_x-146)<1e-8,'center preserving zoom x'); dump('zoomed',s)
    love.keypressed('escape')
  elseif frame==6 then
    check(#s.focus==0 and s.cam_x==0 and s.zoom==1.1,'escape returns root and retains zoom'); dump('back',s)
    love.mousemoved(85.8,115.5); love.mousepressed(85.8,115.5,1,false,1)
  elseif frame==7 then
    check(G.name(G.get(s.selected,'id'))=='source','transformed click hits rendered source'); dump('selected',s)
    love.wheelmoved(0,-2)
  elseif frame==8 then
    check(math.abs(s.cam_y-96/1.1)<1e-8,'wheel pans at zoom'); dump('wheel',s)
    local f=assert(io.open(os.getenv('UML_VIEWER_EVIDENCE')..'/navigation-pass.txt','wb')); f:write('PASS native callbacks: enter, pan, zoom, back, transformed hit, wheel\n'); f:close()
  end
end
