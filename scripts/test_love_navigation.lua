-- Runs only in the isolated acceptance process via UML_VIEWER_TEST_DRIVER.
-- Drives the real LOVE callbacks (and through them imgui input capture):
-- fit on load, double-click drill with fit, control+wheel cursor zoom, blank
-- drag pan, control+0 fit, escape, transformed click and wheel pan.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local Events=require('uml.application.events')
local Hit=require('uml.engine.hit')
-- A failed probe quits before imgui.NewFrame, and the native exit can then lose
-- stderr; record the failing check in the evidence directory first.
local function check(ok,message)
  if not ok then local f=io.open(os.getenv('UML_VIEWER_EVIDENCE')..'/navigation-failure.txt','wb'); if f then f:write(message); f:close() end end
  assert(ok,message)
end
local function near(a,b) return math.abs(a-b)<1e-6 end
local function dump(label,s)
  local f=assert(io.open(os.getenv('UML_VIEWER_EVIDENCE')..'/navigation-'..label..'.edn','wb'))
  f:write(E.encode(G.map{focus=s.focus,zoom=s.zoom or 1,['cam-x']=s.cam_x or 0,['cam-y']=s.cam_y or 0,selected=s.selected or E.nil_value})); f:close()
end
-- Screen point a few px inside a class box under the current camera.
local function on_class(s,id)
  local r=G.get(Hit.class_by_id(s.scene,E.keyword(id)),'rect'); local z=Events.zoom_of(s)
  return (G.get(r,'x')+6-s.cam_x)*z,(G.get(r,'y')+6-s.cam_y)*z
end
local function with_control(f)
  local old=love.keyboard.isDown
  love.keyboard.isDown=function(...) for i=1,select('#',...) do if select(i,...)=='lctrl' then return true end end; return old(...) end
  local ok,err=pcall(f); love.keyboard.isDown=old; assert(ok,err)
end
local before,anchor
return function(frame,s,services)
  if frame==2 then
    check(#s.focus==0 and not s.fit_pending,'initial root is fitted'); dump('root',s)
    local x,y=on_class(s,'source'); love.mousemoved(x,y); love.mousepressed(x,y,1,false,2)
  elseif frame==3 then
    check(E.equal(s.focus,E.parse('[:source]')),'double click enters source')
    check(not s.fit_pending,'drill fitted before the next frame'); dump('entered',s)
    anchor={300,260}; before={Events.world_xy(s,anchor[1],anchor[2])}; before.zoom=s.zoom
    love.mousemoved(anchor[1],anchor[2]); with_control(function() love.wheelmoved(0,1) end)
  elseif frame==4 then
    check(near(s.zoom,math.min(10,before.zoom*1.1)),'control+wheel zooms one 1.1 step')
    local x,y=Events.world_xy(s,anchor[1],anchor[2]); check(near(x,before[1]) and near(y,before[2]),'zoom keeps the cursor world point')
    dump('zoomed',s)
    -- Blank canvas below every box: drag pans by the screen delta over zoom.
    before={cam_x=s.cam_x,cam_y=s.cam_y,zoom=s.zoom}
    local bx,by=Events.world_xy(s,900,860); check(Hit.at(s.scene,bx,by)==nil,'drag starts on blank canvas')
    love.mousepressed(900,860,1,false,1); love.mousemoved(800,810); love.mousereleased(800,810,1)
  elseif frame==5 then
    check(near(s.cam_x,before.cam_x+100/before.zoom) and near(s.cam_y,before.cam_y+50/before.zoom),'blank drag pans'); check(s.drag==nil,'release ends the drag')
    dump('panned',s)
    with_control(function() love.keypressed('0') end)
  elseif frame==6 then
    local f=Events.fit(s,require('uml.application.inspector').canvas_dims(love.graphics.getWidth(),love.graphics.getHeight()))
    check(near(s.zoom,f.zoom) and near(s.cam_x,f.cam_x) and near(s.cam_y,f.cam_y),'control+0 fits the usable canvas'); dump('fitted',s)
    love.keypressed('escape')
  elseif frame==7 then
    check(#s.focus==0,'escape returns to the root'); dump('back',s)
    local x,y=on_class(s,'source'); love.mousemoved(x,y); love.mousepressed(x,y,1,false,1); love.mousereleased(x,y,1)
  elseif frame==8 then
    check(G.name(G.get(s.selected,'id'))=='source','transformed click hits rendered source'); dump('selected',s)
    before={cam_y=s.cam_y}; love.mousemoved(300,300); love.wheelmoved(0,-2)
  elseif frame==9 then
    check(s.cam_y>=before.cam_y,'wheel pans down'); dump('wheel',s)
    before={cam_x=s.cam_x,cam_y=s.cam_y,zoom=s.zoom,selected=E.encode(s.selected)}
    -- Shim only the binding's capture query in this owned process. The actual
    -- callback still forwards input to imgui, but must leave the canvas alone.
    local binding=services.imgui.love; local mouse,key=binding.GetWantCaptureMouse,binding.GetWantCaptureKeyboard
    binding.GetWantCaptureMouse=function() return true end; binding.GetWantCaptureKeyboard=function() return true end
    love.mousepressed(1000,800,1,false,1); love.mousemoved(950,750); love.mousereleased(950,750,1)
    with_control(function() love.wheelmoved(0,1); love.keypressed('=') end)
    binding.GetWantCaptureMouse=mouse; binding.GetWantCaptureKeyboard=key
  elseif frame==10 then
    check(near(s.cam_x,before.cam_x) and near(s.cam_y,before.cam_y) and near(s.zoom,before.zoom) and E.encode(s.selected)==before.selected,'imgui capture blocks canvas input')
    local f=assert(io.open(os.getenv('UML_VIEWER_EVIDENCE')..'/navigation-pass.txt','wb')); f:write('PASS native callbacks: fit, drill, cursor zoom, drag pan, fit key, back, transformed hit, wheel, imgui capture\n'); f:close()
  end
end
