-- Runs inside an isolated actual LOVE viewer (#84) through the probe boundary.
-- Phase "first": DPI-scaled fonts/sidebar, pointer hit, edge drag, same-diagram
-- display, window resize, then :quit-for-restart. Phase "restart": the saved
-- window size, inspector width and view come back. Phase "legacy": an older
-- session.edn without window fields opens with the compatible defaults.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local Display=require('uml.domain.display')
local Atomic=require('uml.adapters.atomic_file')
local root=os.getenv('UML_VIEWER_PROJECT_ROOT')
local evidence=os.getenv('UML_VIEWER_EVIDENCE')
local phase=os.getenv('UML_DPI_TEST_PHASE')
local scale=tonumber(os.getenv('UML_VIEWER_TEST_DPI_SCALE'))
local tag=os.getenv('UML_DPI_TEST_TAG') or 'run'
local function near(a,b,what) assert(math.abs(a-b)<=0.51,what..': expected '..tostring(b)..' got '..tostring(a)) end
local function queue(text) Atomic.write(root..'/.uml-viewer/to-viewer.edn',text) end
local function record(name,value) Atomic.write(evidence..'/'..tag..'-'..name..'.edn',E.encode(value)) end
local function shot(name) love.graphics.captureScreenshot(function(data) local f=assert(io.open(evidence..'/'..tag..'-'..name..'.png','wb')); f:write(data:encode('png'):getString()); f:close() end) end
-- The summary child starts at the sidebar's left edge plus 16 view units of padding.
local function check_sidebar(hooks,d,what)
  local card=assert(hooks.inspector.bounds()['summary-card'],'summary card not drawn')
  near(card.left,d.canvas_px+16*d.ui,what..' summary left')
  assert(card.right<=d.window_w+0.5,what..' summary inside window')
  near(hooks.inspector.width(),d.sidebar_px,what..' sidebar width')
end
local target
return function(frame,state,hooks)
  local d=hooks.display()
  if frame==3 then
    assert(d.ui==scale,'ui scale '..tostring(d.ui)..' for '..tostring(scale))
    near(hooks.fonts.body.FontSize,18*scale,'body font raster')
    near(hooks.fonts.badge.FontSize,9*scale,'badge font raster')
    near(hooks.imgui.GetIO().FontGlobalScale,1,'global font scale')
  end
  if phase=='legacy' then
    if frame==4 then
      local w,h=love.window.getMode(); local dw,dh=love.window.getDesktopDimensions()
      local ew,eh=Display.window_size(nil,nil,d.ui,dw,dh)
      near(w,ew,'legacy window w'); near(h,eh,'legacy window h')
      assert(d.inspector_w==280,'legacy inspector width '..d.inspector_w)
      check_sidebar(hooks,d,'legacy')
      assert(state.zoom==1.25 and state.cam_x==7,'legacy view kept')
      record('pass',G.map{phase=phase,window=E.vector{w,h},inspector=d.inspector_w})
      love.event.quit(0)
    end
    return
  end
  if phase=='restart' then
    if frame==4 then
      local f=assert(io.open(root..'/.uml-viewer/session.edn','rb')); local saved=E.parse(f:read('*a')); f:close()
      local w,h=love.window.getMode()
      near(w,G.get(saved,'window-w'),'restored window w'); near(h,G.get(saved,'window-h'),'restored window h')
      assert(d.inspector_w==G.get(saved,'inspector-width'),'restored inspector width '..d.inspector_w)
      near(d.sidebar_px,G.get(saved,'inspector-width')*d.ui,'restored sidebar px')
      check_sidebar(hooks,d,'restart')
      -- Restoration keeps the saved camera; nothing re-fits the diagram over it.
      assert(state.zoom==1.7 and state.cam_x==33 and state.cam_y==21,'restored view '..tostring(state.zoom)..' '..tostring(state.cam_x))
      record('pass',G.map{phase=phase,window=E.vector{w,h},inspector=d.inspector_w})
      love.event.quit(0)
    end
    return
  end
  if frame==3 then
    near(d.sidebar_px,280*scale,'default sidebar px')
    check_sidebar(hooks,d,'default')
    for _,c in ipairs(G.get(state.scene,'classes') or {}) do
      local r=G.get(c,'rect')
      if r and not G.get(c,'dummy?') then
        local x,y=Display.world_to_window(d,state,G.get(r,'x')+6,G.get(r,'y')+6)
        if Display.in_canvas(d,x) and y<d.window_h then target={id=G.get(c,'id'),x=x,y=y}; break end
      end
    end
    assert(target,'no class inside the canvas')
    love.mousemoved(target.x,target.y,0,0)
  elseif frame==5 then love.mousepressed(target.x,target.y,1,false,1)
  elseif frame==6 then love.mousereleased(target.x,target.y,1,false,1)
  elseif frame==7 then
    local sel=state.selected
    assert(sel and (E.equal(G.get(sel,'id'),target.id) or E.equal(G.get(sel,'parent'),target.id)),'pointer hit missed the drawn class')
    target.edge=d.canvas_px; love.mousemoved(target.edge,200,0,0)
  elseif frame==9 then love.mousepressed(target.edge,200,1,false,1)
  elseif frame==10 then love.mousemoved(target.edge-60*scale,200,-60*scale,0)
  elseif frame==11 then love.mousereleased(target.edge-60*scale,200,1,false,1)
  elseif frame==13 then
    assert(d.inspector_w==340,'dragged inspector width '..d.inspector_w)
    check_sidebar(hooks,d,'dragged')
    near(Display.dims(d).view_w,d.canvas_px/d.ui,'dragged canvas extent')
    queue('{:next-id 2 :queue [{:id 1 :op :display :path "diagram.edn"}]}')
  elseif frame==17 then
    assert(state.mail_seen==1,'same-diagram display not consumed')
    assert(d.inspector_w==340,'display reset the inspector width')
    target.w,target.h=math.floor(1100*scale),math.floor(700*scale)
    love.window.updateMode(target.w,target.h,{resizable=true,highdpi=true})
  elseif frame==21 then
    near(d.window_w,target.w,'resized window w'); near(d.window_h,target.h,'resized window h')
    check_sidebar(hooks,d,'resized')
    local regen=assert(hooks.inspector.bounds().regen,'regen not drawn')
    assert(regen.bottom<=target.h and regen.bottom>=target.h-48*scale,'inspector does not follow the window height')
    shot('zoom1')
  elseif frame==23 then state.zoom=1.7; state.cam_x=33; state.cam_y=21
  elseif frame==25 then shot('zoom17')
  elseif frame==27 then
    record('pass',G.map{phase=phase,chrome=G.map(hooks.chrome())})
    queue('{:next-id 3 :queue [{:id 2 :op :quit-for-restart}]}')
  end
end
