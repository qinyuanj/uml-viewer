-- Isolated native callback acceptance. No desktop-wide keyboard/mouse input.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local before,sources
return function(frame,s,ui)
  local io=ui.imgui.GetIO(); local imgui=ui.imgui
  local function check(ok,message)
    if ok then return end
    local f=assert(_G.io.open(os.getenv('UML_VIEWER_EVIDENCE')..'/search-failure.txt','wb')); f:write(message); f:close()
    error(message)
  end
  local function control(fn)
    local old=love.keyboard.isDown
    love.keyboard.isDown=function(...) for i=1,select('#',...) do if select(i,...)=='lctrl' then return true end end; return false end
    fn(); love.keyboard.isDown=old
  end
  if frame==2 then
    check(io.IniFilename==nil,'native test ini isolation')
    control(function() love.keypressed('f') end)
  elseif frame==4 then
    check(ui.inspector.search_active(),'Ctrl+F focuses search')
    love.textinput('解析')
  elseif frame==6 then
    check(ui.inspector.search_query()=='解析','Chinese textinput reaches search')
    check(ui.inspector.bounds()['search-result:alpha.deep.parser']~=nil,'deep target appears')
    check(ui.inspector.bounds()['search-result:beta.parser']~=nil,'same-name target is disambiguated')
    before={focus=E.encode(s.focus),zoom=s.zoom,cam_x=s.cam_x,cam_y=s.cam_y}
    love.keypressed('p'); love.keypressed('r'); control(function() love.keypressed('=') end)
    io:AddKeyEvent(imgui.ImGuiKey_DownArrow,true)
  elseif frame==7 then io:AddKeyEvent(imgui.ImGuiKey_DownArrow,false)
  elseif frame==9 then
    check(before.focus==E.encode(s.focus) and before.zoom==s.zoom and before.cam_x==s.cam_x and before.cam_y==s.cam_y,'search captures canvas shortcuts')
    check(E.equal(ui.inspector.search_selection(),E.keyword('alpha.deep.parser')),'Down selects the first explicit result')
    io:AddKeyEvent(imgui.ImGuiKey_Enter,true)
  elseif frame==10 then io:AddKeyEvent(imgui.ImGuiKey_Enter,false)
  elseif frame==12 then
    check(E.equal(s.focus,E.parse('[:alpha :deep]')),'Enter enters target namespace; focus='..E.encode(s.focus)..' selection='..E.encode(ui.inspector.search_selection())..' active='..tostring(ui.inspector.search_active())..' query='..ui.inspector.search_query())
    check(E.equal(s.detail_id,E.keyword('alpha.deep.parser')),'Enter opens target details')
    check(E.equal(G.get(s.selected,'id'),s.detail_id),'target selected for dependency highlighting')
    -- Exercise the actual independent native source windows with an isolated source boundary.
    local Source=require('uml.adapters.source_window')
    sources=Source.new(imgui,{fonts=ui.fonts,source={member_source=function(ident) return {body='local value = 1\nreturn value',line=1,title=ident.ns,file=ident.ns..'.lua'} end}})
    assert(sources.open{ns='first',lang='lua'})
    assert(sources.open{ns='second',lang='lua'})
    check(#sources.windows()==2,'two source windows coexist')
    check(sources.windows()[1].view().title=='first','opening second retains first')
    local draw=love.draw
    love.draw=function() local original=ui.source_window; sources.draw(); draw() end
  elseif frame==15 then
    -- Windows are drawn through an owned adapter; close one independently.
    sources.close(1)
    check(#sources.windows()==1 and sources.view().title=='second','closing one retains the other source window')
    local f=assert(_G.io.open(os.getenv('UML_VIEWER_EVIDENCE')..'/search-pass.txt','wb')); f:write('PASS Ctrl+F, Chinese input, explicit Down selection, Enter cross-layer locate, input capture, independent source windows, ini isolation\n'); f:close()
    love.graphics.captureScreenshot(function(data) local f=assert(_G.io.open(os.getenv('UML_VIEWER_EVIDENCE')..'/search.png','wb')); f:write(data:encode('png'):getString()); f:close() end)
  elseif frame==17 then love.event.quit(0) end
end
