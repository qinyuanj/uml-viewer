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
    assert(font==ui.fonts.body,'inspector text must use the 16px body font')
    local expect=os.getenv('UML_FONT_TEST_EXPECT') or 'sarasa'
    assert(ui.fonts.source==expect,'inspector font source '..tostring(ui.fonts.source)..' expected '..expect)
    for _,f in ipairs{ui.fonts.body,ui.fonts.title} do
      for _,code in ipairs{0x4e2d,0x6587,0x65b9,0x6848,0x8f93,0x5165} do
        assert(f:FindGlyphNoFallback(code)~=nil,'missing CJK glyph '..code)
      end
    end
    local proposal=Events.show_proposal(state,E.keyword('plan'))
    assert(G.get(proposal.extra,'proposal')==true,'proposal view not shown')
    local real=Events.on_key(proposal,'p')
    assert(not G.get(real.extra,'proposal-id') and not G.get(real.extra,'proposal'),'P must restore the namespace tree')
    assert(#real.focus==0,'P clears proposal drill focus')
    local f=assert(io.open(os.getenv('UML_VIEWER_EVIDENCE')..'/inspector-pass.txt','wb'))
    f:write('PASS native Cimgui proposal-name field received 中文方案输入 via LOVE textinput; '..ui.fonts.source..' glyphs verified at 16px and 20px; P returns namespace tree\n'); f:close()
    love.graphics.captureScreenshot(function(data) local out=assert(io.open(os.getenv('UML_VIEWER_EVIDENCE')..'/inspector.png','wb')); out:write(data:encode('png'):getString()); out:close() end)
  end
end
