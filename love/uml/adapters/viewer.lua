-- LOVE callback installation; owns no companion, mailbox or existing viewer resources.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local IR=require('uml.domain.ir')
local D=require('uml.application.document')
local Events=require('uml.application.events')
local Paths=require('uml.adapters.windows_path')
local Watch=require('uml.adapters.file_watch')
local Boxes=require('uml.adapters.class_boxes')
local Edges=require('uml.adapters.edges')
local Metrics=require('uml.adapters.metrics')
local Inspector=require('uml.adapters.inspector')
local InspectorModel=require('uml.application.inspector')
local Mail=require('uml.adapters.mail')
local Menu=require('uml.adapters.mail_menu')
local SessionFile=require('uml.adapters.session')
local Session=require('uml.application.session')
local ProcessIdentity=require('uml.adapters.process_identity')
local M={}
local function read(path) local f=assert(io.open(path,'rb')); local s=f:read('*a'); f:close(); return s end
local function parse_args(args)
  local path,restart
  for _,value in ipairs(args) do
    if value=='--restart' then restart=true
    elseif value:sub(1,1)=='-' then error('usage: uml --runtime love [--restart] [diagram.edn]')
    elseif not path then path=value end
  end
  return path,restart==true
end
function M.run(args,services)
  services=services or {}
  local explicit_path,restart=parse_args(args)
  local path=explicit_path or 'examples/uml-viewer.edn'
  local dependencies=os.getenv('UML_CIMGUI_DIRECTORY')
  if dependencies then dependencies=dependencies:gsub('\\','/'); package.path=dependencies..'/?.lua;'..dependencies..'/?/init.lua;'..package.path; package.cpath=dependencies..'/?.dll;'..package.cpath end
  local imgui=require('cimgui')
  local metrics=Metrics.new()
  local state,renderer,inspector,source_window,mail,elapsed,frames= nil,nil,nil,nil,nil,0,0
  local function load_document(p)
    local snapshots,stamp=metrics.snapshot(metrics.root(p))
    local s=D.new(IR.read_document(read(p)),Watch.mtime(p),nil,nil,snapshots,stamp)
    s.path=p; s.project_root=metrics.root(p); return s
  end
  local evidence=os.getenv('UML_VIEWER_EVIDENCE')
  local test_seconds=tonumber(os.getenv('UML_VIEWER_TEST_SECONDS'))
  local probe_path=os.getenv('UML_VIEWER_TEST_DRIVER')
  local probe=probe_path and assert(loadfile(probe_path))()
  local function publish()
    if evidence then local f=assert(io.open(evidence..'/scene-'..state.revision..'.edn','wb')); f:write(E.encode(state.scene)); f:close() end
  end
  local project_root=os.getenv('UML_VIEWER_PROJECT_ROOT')
  project_root=project_root and #project_root>0 and project_root or '.'
  project_root=Paths.canonical(project_root)
  function love.load()
    love.window.setTitle('UML viewer — LOVE')
    love.window.setMode(1100,760,{resizable=true})
    imgui.love.Init(); Inspector.fonts(imgui); imgui.love.BuildFontAtlas(); renderer=Boxes.new(imgui)
    local snapshot=restart and SessionFile.read(project_root) or nil
    path=Paths.canonical(Session.path(explicit_path,snapshot,path))
    state=load_document(path)
    state.project_root=project_root
    if restart then state=Session.restore(state,snapshot) end
    mail=Mail.new(state.project_root,{load=load_document,canonical=Paths.canonical,resolve=function(p,root) p=p:gsub('\\','/'); return Paths.canonical((p:match('^%a:/') or p:sub(1,1)=='/') and p or (root..'/'..p)) end})
    if services.source then source_window=require('uml.adapters.source_window').new(imgui,{root=state.project_root,source=services.source}) end
    inspector=Inspector.new(imgui,{
      choose=function(id) local next_state,context=InspectorModel.choose(state,id); state=context and mail.send_context(next_state,context) or next_state end,
      pick=function(id) state.selected=G.map{kind=E.keyword('class'),id=id} end,
      open_source=function(ident) if source_window then source_window.open(ident) end end})
    -- A refused or invalid startup never overwrites an existing identity.
    ProcessIdentity.record(project_root)
    publish()
  end
  function love.update(dt)
    elapsed=elapsed+dt; frames=frames+1
    local ok,err=pcall(function()
      state=mail.poll(state); path=state.path or path
      if state.quit_for_restart then
        SessionFile.save(state.project_root,Session.snapshot(state))
        love.event.quit(0); return
      end
      local stamp=Watch.mtime(path)
      local snapshots,metrics_stamp=metrics.snapshot(metrics.root(path))
      if stamp~=state.mtime or metrics_stamp~=state.metrics_stamp then
        local next_state,changed,rejected=D.reload(state,read(path),stamp,snapshots,metrics_stamp)
        state=next_state; if changed then publish() elseif rejected then error(rejected) end
      end
    end)
    if not ok then io.stderr:write('Document reload rejected: '..tostring(err)..'\n') end
    if probe then
      local passed,reason=pcall(probe,frames,state,{inspector=inspector,source_window=source_window,imgui=imgui,mail=mail})
      if not passed then io.stderr:write('Navigation acceptance failed: '..tostring(reason)..'\n'); love.event.quit(1); return end
    end
    imgui.love.Update(dt); imgui.NewFrame()
    if test_seconds and elapsed>=test_seconds then love.event.quit(0) end
  end
  function love.draw()
    love.graphics.clear(.09,.10,.12,1)
    local width,height=love.graphics.getWidth(),love.graphics.getHeight()
    local scale=Events.zoom_of(state)
    Edges.draw(imgui,imgui.GetBackgroundDrawList_Nil(),state.scene,-(state.cam_x or 0)*scale,-(state.cam_y or 0)*scale,scale)
    renderer.draw(state.scene,width,height,state)
    inspector.draw(state)
    Menu.draw(imgui,state,state.selected,function(op,extra) mail.request(op,extra) end)
    if source_window then source_window.draw() end
    if #state.focus>0 or G.get(state.extra,'open-layer') then love.graphics.setColor(1,1,1,1); love.graphics.print('< Back',12,12) end
    imgui.Render(); imgui.love.RenderDrawLists()
    if evidence and frames==4 then love.graphics.captureScreenshot(function(data) local f=assert(io.open(evidence..'/render.png','wb')); f:write(data:encode('png'):getString()); f:close() end) end
  end
  function love.quit() imgui.love.Shutdown() end
  function love.textinput(t) imgui.love.TextInput(t) end
  local function dims() return {window_w=love.graphics.getWidth(),window_h=love.graphics.getHeight(),control=love.keyboard.isDown('lctrl','rctrl')} end
  function love.keypressed(k)
    imgui.love.KeyPressed(k)
    if not imgui.love.GetWantCaptureKeyboard() then state=Events.on_key(state,k,dims()) end
  end
  function love.mousemoved(x,y) if not imgui.love.GetWantCaptureMouse() then state=Events.on_move(state,x,y) end end
  function love.mousepressed(x,y,button,istouch,presses)
    imgui.love.MousePressed(button)
    if button==2 and not imgui.love.GetWantCaptureMouse() then state=Events.on_press(state,x,y); if state.selected then imgui.OpenPopup_Str('uml-element-mail') end end
    if button==1 and not imgui.love.GetWantCaptureMouse() then
      local back=(#state.focus>0 or G.get(state.extra,'open-layer')) and y<44 and x<320
      state=Events.on_press(state,x,y)
      if not back and (presses or 1)>=2 then local id=Events.layer_id(state.selected); if id then state=Events.drill(state,id) end end
    end
  end
  function love.mousereleased(x,y,button) imgui.love.MouseReleased(button) end
  function love.wheelmoved(x,y)
    imgui.love.WheelMoved(x,y)
    if not imgui.love.GetWantCaptureMouse() then
      local d=dims(); d.horizontal=x~=0 or love.keyboard.isDown('lshift','rshift')
      state=Events.on_scroll(state,x~=0 and -x or -y,d)
    end
  end
  function love.keyreleased(k) imgui.love.KeyReleased(k) end
end
return M
