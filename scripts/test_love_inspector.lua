-- Native rename flow: no permanently visible proposal input in the summary.
local G=require('uml.domain.geom')
local E=require('uml.edn')
local Events=require('uml.application.events')
local cancel_label,isolation_failure,button_failure
return function(frame,state,ui)
  local gui_io=ui.imgui.GetIO()
  if frame==1 then
    local isolated=gui_io.IniFilename==nil
    gui_io.IniFilename=nil -- The red run must also protect the user's settings.
    isolation_failure=not isolated
    local button=ui.imgui.Button
    ui.imgui.Button=function(label,...)
      if label:find('##cancel-rename',1,true) then cancel_label=label end
      if label:find('##new-proposal',1,true) or label:find('##declutter',1,true) or label:find('##regen',1,true) then
        local style=ui.imgui.GetStyle()
        local bg,ink=style.Colors[ui.imgui.ImGuiCol_Button],style.Colors[ui.imgui.ImGuiCol_Text]
        if math.abs(bg.x*255-42)>1 or math.abs(bg.y*255-61)>1 or math.abs(bg.z*255-54)>1
          or math.abs(ink.x*255-232)>1 or math.abs(ink.y*255-196)>1 or math.abs(ink.z*255-72)>1 then
          button_failure='sidebar buttons must use original green fill and gold ink'
        end
      end
      return button(label,...)
    end
  end
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
    if isolation_failure or button_failure or cancel_label~='取消##cancel-rename' then
      local f=assert(io.open(os.getenv('UML_VIEWER_EVIDENCE')..'/acceptance-fail.txt','wb'))
      f:write(isolation_failure and 'native tests must disable ImGui ini before the first frame' or button_failure or 'rename must offer 取消, got '..tostring(cancel_label)); f:close()
      return love.event.quit(1)
    end
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
