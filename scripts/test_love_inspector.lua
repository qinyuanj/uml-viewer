-- Native rename flow: no permanently visible proposal input in the summary.
local G=require('uml.domain.geom')
local E=require('uml.edn')
local Events=require('uml.application.events')
return function(frame,state,ui)
  local gui_io=ui.imgui.GetIO()
  local function point(key)
    local b=assert(ui.inspector.bounds()[key],'missing '..key)
    gui_io:AddMousePosEvent(b.x,b.y)
    return b
  end
  if frame==2 then point('proposal:plan')
  elseif frame==3 then gui_io:AddMouseButtonEvent(1,true)
  elseif frame==4 then gui_io:AddMouseButtonEvent(1,false)
  elseif frame==6 then point('rename-menu')
  elseif frame==7 then gui_io:AddMouseButtonEvent(0,true)
  elseif frame==8 then gui_io:AddMouseButtonEvent(0,false)
  elseif frame==10 then
    assert(ui.inspector.bounds()['proposal-name'],'rename must reveal the input')
    gui_io:AddKeyEvent(ui.imgui.ImGuiMod_Ctrl,true); gui_io:AddKeyEvent(ui.imgui.ImGuiKey_A,true)
  elseif frame==11 then
    gui_io:AddKeyEvent(ui.imgui.ImGuiKey_A,false); gui_io:AddKeyEvent(ui.imgui.ImGuiMod_Ctrl,false)
  elseif frame==12 then love.event.push('textinput','中文方案输入')
  elseif frame==16 then
    assert(ui.inspector.name()=='中文方案输入','rename field must receive Chinese textinput')
    assert(ui.fonts.source==(os.getenv('UML_FONT_TEST_EXPECT') or 'sarasa'))
    for _,f in ipairs{ui.fonts.body,ui.fonts.title,ui.fonts.badge,ui.fonts.level,ui.fonts.marker,ui.fonts.code} do
      for _,code in ipairs{0x4e2d,0x6587,0x65b9,0x6848,0x8f93,0x5165} do assert(f:FindGlyphNoFallback(code)~=nil,'missing CJK glyph '..code) end
    end
    local p=Events.show_proposal(state,E.keyword('plan'))
    assert(G.get(p.extra,'proposal')==true)
    local real=Events.on_key(p,'p'); assert(not G.get(real.extra,'proposal-id') and #real.focus==0)
    local f=assert(io.open(os.getenv('UML_VIEWER_EVIDENCE')..'/inspector-pass.txt','wb'))
    f:write('PASS native proposal context menu -> rename -> Chinese textinput; Sarasa 9/11/12/13/18 glyphs; P returns namespace tree\n'); f:close()
    love.graphics.captureScreenshot(function(data) local out=assert(io.open(os.getenv('UML_VIEWER_EVIDENCE')..'/inspector.png','wb')); out:write(data:encode('png'):getString()); out:close() end)
  elseif frame==18 then love.event.quit(0) end
end
