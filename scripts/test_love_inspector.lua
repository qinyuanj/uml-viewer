-- Runs only in a newly launched native viewer with a temporary document.
local G=require('uml.domain.geom')
local E=require('uml.edn')
local Events=require('uml.application.events')
return function(frame,state,ui)
  if frame==2 then ui.inspector.focus_name() end
  if frame==4 then love.event.push('textinput','中文方案输入') end
  if frame==7 then
    local got=ui.inspector.name()
    assert(got=='中文方案输入','native textinput must populate the actual proposal field')
    local font=ui.imgui.GetFont(); assert(font~=nil,'focused font missing')
    for _,code in ipairs{0x4e2d,0x6587,0x65b9,0x6848,0x8f93,0x5165} do
      assert(font:FindGlyphNoFallback(code)~=nil,'missing CJK glyph '..code)
    end
    local proposal=Events.show_proposal(state,E.keyword('plan'))
    assert(G.get(proposal.extra,'proposal')==true,'proposal view not shown')
    local real=Events.on_key(proposal,'p')
    assert(not G.get(real.extra,'proposal-id') and not G.get(real.extra,'proposal'),'P must restore the namespace tree')
    assert(#real.focus==0,'P clears proposal drill focus')
    local f=assert(io.open(os.getenv('UML_VIEWER_EVIDENCE')..'/inspector-pass.txt','wb'))
    f:write('PASS native Cimgui proposal-name field received 中文方案输入 via LOVE textinput; YaHei glyphs verified; P returns namespace tree\n'); f:close()
    love.graphics.captureScreenshot(function(data) local out=assert(io.open(os.getenv('UML_VIEWER_EVIDENCE')..'/inspector.png','wb')); out:write(data:encode('png'):getString()); out:close() end)
  end
end
