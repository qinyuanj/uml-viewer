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
local DetailWindow=require('uml.adapters.detail_window')
local Fonts=require('uml.adapters.fonts')
local InspectorModel=require('uml.application.inspector')
local Hit=require('uml.engine.hit')
local Mail=require('uml.adapters.mail')
local Menu=require('uml.adapters.mail_menu')
local SessionFile=require('uml.adapters.session')
local Session=require('uml.application.session')
local ProcessIdentity=require('uml.adapters.process_identity')
local Worker=require('uml.adapters.companion_worker')
local Mailbox=require('uml.adapters.mailbox')
local PolicyFile=require('uml.adapters.policy_file')
local T=require('uml.domain.theme')
local M={}
local function read(path) local f=assert(io.open(path,'rb')); local s=f:read('*a'); f:close(); return s end
local function parse_args(args)
  local path,restart,backend
  local index=1
  while index<=#args do
    local value=args[index]
    if value=='--restart' then restart=true
    elseif value=='--backend' then
      local selected=args[index+1]
      if selected~='herdr' then error('Unknown or duplicate companion backend; use --backend herdr') end
      backend=selected; index=index+1
    elseif value:sub(1,1)=='-' then error('usage: uml [--backend herdr] [--restart] [diagram.edn]')
    elseif not path then path=value end
    index=index+1
  end
  return path,restart==true,backend
end
function M.run(args,services)
  services=services or {}
  local explicit_path,restart,backend=parse_args(args)
  local path=explicit_path or 'examples/uml-viewer.edn'
  local dependencies=os.getenv('UML_CIMGUI_DIRECTORY')
  if dependencies then dependencies=dependencies:gsub('\\','/'); package.path=dependencies..'/?.lua;'..dependencies..'/?/init.lua;'..package.path; package.cpath=dependencies..'/?.dll;'..package.cpath end
  local imgui=require('cimgui')
  local metrics=Metrics.new()
  local state,renderer,inspector,source_window,mail,fonts,elapsed,frames= nil,nil,nil,nil,nil,nil,0,0
  local detail_window
  -- The usable canvas: the window minus the fixed sidebar.
  local function canvas_dims() return InspectorModel.canvas_dims(love.graphics.getWidth(),love.graphics.getHeight()) end
  local companion,keeping_companion,lifecycle_options=nil,false,nil
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
    love.window.setMode(1500,920,{resizable=true})
    imgui.love.Init(); fonts=Fonts.load(imgui,Fonts.defaults(project_root)); imgui.love.BuildFontAtlas(); renderer=Boxes.new(imgui,fonts)
    local snapshot=restart and SessionFile.read(project_root) or nil
    path=Paths.canonical(Session.path(explicit_path,snapshot,path))
    local mail_seen
    if backend then
      -- Capture the viewer cursor before the companion exists so a :display
      -- written during startup is consumed, not skipped.
      mail_seen=Mailbox.open(project_root..'/.uml-viewer/to-viewer.edn').last_id()
      companion=Worker.start(project_root,restart)
    end
    if backend and not restart then
      state=D.waiting_state(path,mail_seen)
      state.project_root=project_root; state.revision=1
      state.focus=E.vector(); state.extra=E.map(); state.cam_x=0; state.cam_y=0; state.zoom=1
    else
      state=load_document(path)
      state.project_root=project_root
      -- A fresh load fits the canvas; a restart restores its saved view instead.
      if restart then state=Session.restore(state,snapshot) else state.fit_pending=true end
    end
    local to_agent=Mailbox.open(project_root..'/.uml-viewer/to-agent.edn')
    local function notify_agent(cmd)
      if companion then Worker.command(companion,'wake',{id=G.get(cmd,'id')}) end
    end
    mail=Mail.new(state.project_root,{load=load_document,canonical=Paths.canonical,notify=notify_agent,resolve=function(p,root) p=p:gsub('\\','/'); return Paths.canonical((p:match('^%a:/') or p:sub(1,1)=='/') and p or (root..'/'..p)) end})
    if services.source then source_window=require('uml.adapters.source_window').new(imgui,{root=state.project_root,source=services.source,fonts=fonts}) end
    inspector=Inspector.new(imgui,{
      choose=function(id) local next_state,context=InspectorModel.choose(state,id); state=context and mail.send_context(next_state,context) or next_state end,
      new_proposal=function() state=InspectorModel.press(state,G.map{kind=E.keyword('new-proposal')},{boundary=PolicyFile}); publish() end,
      declutter=function() state=InspectorModel.press(state,G.map{kind=E.keyword('declutter')}); publish() end,
      regen=function()
        local ok=pcall(mail.request,'regen',G.map{})
        state.mail_status=ok and (companion and 'Regen requested.' or 'Regen queued; Claude session not attached.') or 'Regen failed.'
      end,
      rename=function(id,name) state=Events.rename_proposal(state,id,name,{boundary=PolicyFile}); publish() end,
      delete=function(id) state=Events.delete_proposal(state,id,{boundary=PolicyFile}); publish() end,
      open_source=function(ident) if source_window then source_window.open(ident) end end},fonts)
    detail_window=DetailWindow.new(imgui,{
      close=function() state=Events.close_detail(state) end,
      pick=function(id) state=Events.open_detail(state,G.map{kind=E.keyword('class'),id=id}) end,
      open_source=function(ident) if source_window then source_window.open(ident) end end},fonts)
    -- A refused or invalid startup never overwrites an existing identity.
    if not companion then ProcessIdentity.record(project_root) end
    publish()
  end
  function love.update(dt)
    elapsed=elapsed+dt; frames=frames+1
    if companion then
      local previous=companion.seen
      local polled,reason=pcall(Worker.poll,companion)
      if not polled then companion.phase='failed'; companion.message=tostring(reason) end
      if companion.phase=='ready' and not companion.recorded then
        ProcessIdentity.record(project_root); companion.recorded=true
      end
      if companion.seen~=previous or companion.phase=='failed' then
        state.mail_status=companion.message or ('Companion: '..companion.phase)
      end
    end
    local ok,err=pcall(function()
      state=mail.poll(state); path=state.path or path
      if state.quit_for_restart then
        if companion then
          if not companion.restart_stop then
            Worker.command(companion,'stop',{keep=true}); companion.restart_stop=true
          end
          if companion.phase~='stopped' or not companion.exited then return end
          keeping_companion=true
        end
        SessionFile.save(state.project_root,Session.snapshot(state))
        love.event.quit(0); return
      end
      if not state.waiting then
        local stamp=Watch.mtime(path)
        local snapshots,metrics_stamp=metrics.snapshot(metrics.root(path))
        if stamp~=state.mtime or metrics_stamp~=state.metrics_stamp then
          local next_state,changed,rejected=D.reload(state,read(path),stamp,snapshots,metrics_stamp)
          state=next_state; if changed then publish() elseif rejected then error(rejected) end
        end
      end
    end)
    if not ok then io.stderr:write('Document reload rejected: '..tostring(err)..'\n') end
    -- Navigation and new loads request a fit; apply it with the live canvas size.
    if state.fit_pending then state=Events.settle(state,canvas_dims()) end
    if probe then
      local passed,reason=pcall(probe,frames,state,{inspector=inspector,source_window=source_window,detail_window=detail_window,imgui=imgui,mail=mail,fonts=fonts})
      if not passed then io.stderr:write('Navigation acceptance failed: '..tostring(reason)..'\n'); love.event.quit(1); return end
    end
    imgui.love.Update(dt); imgui.NewFrame()
    if test_seconds and elapsed>=test_seconds then love.event.quit(0) end
  end
  function love.draw()
    local bg=T.colors.canvas_bg
    love.graphics.clear(bg[1]/255,bg[2]/255,bg[3]/255,1)
    local width,height=love.graphics.getWidth(),love.graphics.getHeight()
    local scale=Events.zoom_of(state)
    local canvas_w=InspectorModel.canvas_w(width)
    -- Both renderers consume the actual selected/hover hits: class, child row,
    -- port or edge. Shared rules keep the focus and dimming consistent.
    local view={selected=state.selected,hover=state.hover}
    local list=imgui.GetBackgroundDrawList_Nil()
    -- Clip canvas painting to the usable area left of the fixed sidebar.
    list:PushClipRect(imgui.ImVec2_Float(0,0),imgui.ImVec2_Float(canvas_w,height),true)
    Edges.draw(imgui,list,state.scene,-(state.cam_x or 0)*scale,-(state.cam_y or 0)*scale,scale,view)
    renderer.draw(state.scene,width,height,{zoom=scale,cam_x=state.cam_x or 0,cam_y=state.cam_y or 0,selected=state.selected,hover=state.hover})
    list:PopClipRect()
    inspector.draw(state)
    if detail_window then detail_window.draw(state) end
    Menu.draw(imgui,state,state.selected,function(op,extra) mail.request(op,extra) end)
    if source_window then source_window.draw() end
    if #state.focus>0 or G.get(state.extra,'open-layer') then renderer.back() end
    imgui.Render(); imgui.love.RenderDrawLists()
    if evidence and frames==4 then love.graphics.captureScreenshot(function(data) local f=assert(io.open(evidence..'/render.png','wb')); f:write(data:encode('png'):getString()); f:close() end) end
  end
  function love.quit()
    imgui.love.Shutdown()
    if companion and not companion.exited then
      Worker.command(companion,'stop',{keep=keeping_companion})
    end
  end
  function love.textinput(t) imgui.love.TextInput(t) end
  -- Canvas pointer input stops at the fixed sidebar's left edge.
  local function on_canvas(x) return InspectorModel.in_canvas(x,love.graphics.getWidth()) end
  local function dims()
    -- Zoom anchors on state.pointer: the last canvas position from mousemoved.
    local d=canvas_dims()
    d.control=love.keyboard.isDown('lctrl','rctrl'); d.shift=love.keyboard.isDown('lshift','rshift')
    return d
  end
  function love.keypressed(k)
    imgui.love.KeyPressed(k)
    if not imgui.love.GetWantCaptureKeyboard() then state=Events.on_key(state,k,dims()) end
  end
  function love.mousemoved(x,y)
    imgui.love.MouseMoved(x,y)
    -- An active canvas drag keeps panning even over the sidebar or a window.
    if state.drag then state=Events.on_drag(state,x,y)
    elseif not imgui.love.GetWantCaptureMouse() and on_canvas(x) then state=Events.on_move(state,x,y)
    else state=Events.on_leave(state) end
  end
  function love.mousepressed(x,y,button,istouch,presses)
    imgui.love.MousePressed(button)
    if imgui.love.GetWantCaptureMouse() or not on_canvas(x) then return end
    if button==2 then
      state=Events.on_press(state,x,y)
      if Events.element_menu(state.selected) then imgui.OpenPopup_Str('uml-element-mail') end
    elseif button==1 then
      -- Both transitions honour the back hotspot first.
      if (presses or 1)>=2 then state=Events.on_double_press(state,x,y) else state=Events.on_left_press(state,x,y) end
    end
  end
  function love.mousereleased(x,y,button)
    imgui.love.MouseReleased(button)
    -- Release always ends a canvas drag, wherever the pointer now is.
    if button==1 and state.drag then state=Events.on_release(state,x,y) end
  end
  function love.wheelmoved(x,y)
    if detail_window and detail_window.detail_hovered() then
      state=detail_window.wheel(state,-y)
    else
      imgui.love.WheelMoved(x,y)
      if not imgui.love.GetWantCaptureMouse() then state=Events.on_wheel(state,x,y,dims()) end
    end
  end
  function love.keyreleased(k) imgui.love.KeyReleased(k) end
end
return M
