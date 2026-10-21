-- LOVE callback installation; owns no companion, mailbox or existing viewer resources.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local IR=require('uml.domain.ir')
local D=require('uml.application.document')
local Events=require('uml.application.events')
local Paths=require('uml.adapters.platform_path')
local Watch=require('uml.adapters.platform_watch')
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
local Display=require('uml.domain.display')
local Dpi=require('uml.adapters.dpi')
local Polling=require('uml.domain.polling')
local Strings=require('uml.application.strings')
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
  -- Dependency location: explicit override, else the project's provisioned
  -- install copy, discovered from the project root, never from the host OS.
  local project_root_env=os.getenv('UML_VIEWER_PROJECT_ROOT')
  local base=project_root_env and #project_root_env>0 and project_root_env or '.'
  local path=explicit_path or SessionFile.default_diagram(base)
  local dependencies=os.getenv('UML_CIMGUI_DIRECTORY')
  if not dependencies then
    local discovered=base:gsub('\\','/'):gsub('/$','')..'/.uml-viewer/cimgui-love-1.90-1'
    local probe=io.open(discovered..'/cimgui/init.lua','rb')
    if probe then probe:close(); dependencies=discovered end
  end
  if dependencies then dependencies=dependencies:gsub('\\','/'); package.path=dependencies..'/?.lua;'..dependencies..'/?/init.lua;'..package.path; package.cpath=dependencies..'/?.so;'..dependencies..'/?.dll;'..package.cpath end
  local imgui=require('cimgui')
  local metrics=Metrics.new()
  local state,renderer,inspector,source_window,mail,fonts,elapsed,frames= nil,nil,nil,nil,nil,nil,0,0
  local detail_window
  local frame_active=false
  local polling=Polling.new(function() return love.timer.getTime() end)
  local force_refresh,show_performance,frame_ms=false,false,0
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
  -- Window chrome: OS DPI (adapters.dpi), the backend framebuffer scale and
  -- the DPI-independent inspector width; every frame derives one display
  -- record (domain.display) shared by drawing, imgui and pointer input.
  local os_scale,inspector_w,dragging_edge,font_display
  local function display()
    return Display.new{window_w=love.graphics.getWidth(),window_h=love.graphics.getHeight(),
      os_scale=os_scale,framebuffer_scale=love.window.getDPIScale(),inspector_w=inspector_w}
  end
  local function chrome()
    local w,h=love.window.getMode()
    return {window_w=w,window_h=h,inspector_w=display().inspector_w}
  end
  function love.load()
    love.window.setTitle('UML viewer — LOVE')
    local snapshot=restart and SessionFile.read(project_root) or nil
    local saved=Session.chrome(snapshot)
    os_scale=Dpi.os_scale()
    local ui=Display.ui_scale(os_scale,love.window.getDPIScale())
    local desktop_w,desktop_h=love.window.getDesktopDimensions()
    local window_w,window_h=Display.window_size(saved.window_w,saved.window_h,ui,desktop_w,desktop_h)
    love.window.setMode(window_w,window_h,{resizable=true,highdpi=true,
      minwidth=math.floor(Display.window_min.w*ui),minheight=math.floor(Display.window_min.h*ui)})
    -- setMode may move the window to another monitor: re-read its DPI.
    os_scale=Dpi.os_scale() or os_scale
    inspector_w=saved.inspector_w
    imgui.love.Init()
    -- ImGui loads settings at its first NewFrame. Test processes must neither
    -- load nor save the user's layout; normal viewer persistence stays enabled.
    if probe_path or test_seconds then imgui.GetIO().IniFilename=nil end
    local d=display()
    if d.ui~=1 then imgui.GetStyle():ScaleAllSizes(d.ui) end
    local font_opts=Fonts.defaults(project_root); font_opts.display=d
    fonts=Fonts.load(imgui,font_opts); font_display=d
    imgui.love.BuildFontAtlas(); renderer=Boxes.new(imgui,fonts)
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
    if services.source then source_window=require('uml.adapters.source_window').new(imgui,{root=state.project_root,source=services.source,fonts=fonts,ui=function() return display().ui end}) end
    -- A source that cannot be opened says why instead of doing nothing.
    local function open_source(ident)
      if not source_window then return end
      local opened,reason=source_window.open(ident)
      if not opened then state.mail_status=Strings.source_not_opened:format(tostring(reason)) end
    end
    inspector=Inspector.new(imgui,{
      locate=function(id) state=Events.locate(state,id,Display.dims(display())); publish() end,
      choose=function(id) local next_state,context=InspectorModel.choose(state,id); state=context and mail.send_context(next_state,context) or next_state end,
      new_proposal=function() state=InspectorModel.press(state,G.map{kind=E.keyword('new-proposal')},{boundary=PolicyFile}); publish() end,
      declutter=function() state=InspectorModel.press(state,G.map{kind=E.keyword('declutter')}); publish() end,
      regen=function()
        local ok=pcall(mail.request,'regen',G.map{})
        state.mail_status=ok and (companion and 'Regen requested.' or 'Regen queued; Claude session not attached.') or 'Regen failed.'
      end,
      rename=function(id,name) state=Events.rename_proposal(state,id,name,{boundary=PolicyFile}); publish() end,
      delete=function(id) state=Events.delete_proposal(state,id,{boundary=PolicyFile}); publish() end,
      open_source=open_source},fonts)
    detail_window=DetailWindow.new(imgui,{
      close=function() state=Events.close_detail(state) end,
      pick=function(id) state=Events.locate(state,id,Display.dims(display())) end,
      open_source=open_source},fonts)
    -- A refused or invalid startup never overwrites an existing identity.
    if not companion then ProcessIdentity.record(project_root) end
    publish()
  end
  function love.update(dt)
    elapsed=elapsed+dt; frames=frames+1; frame_ms=dt*1000
    -- A per-monitor DPI change preserves the camera and the inspector's view-
    -- unit width. Rebuild only between frames (the previous frame was rendered).
    os_scale=Dpi.os_scale() or os_scale
    local d=display()
    if fonts and (d.ui~=font_display.ui or d.fb~=font_display.fb) then
      imgui.GetStyle():ScaleAllSizes(d.ui/font_display.ui)
      imgui.GetIO().Fonts:Clear()
      local opts=Fonts.defaults(project_root); opts.display=d
      local rebuilt=Fonts.load(imgui,opts)
      for k,v in pairs(rebuilt) do fonts[k]=v end
      Fonts.current=fonts; font_display=d
      imgui.love.BuildFontAtlas()
    end
    -- cimgui-love sets this at Init only; keep it current after monitor changes.
    imgui.GetIO().DisplayFramebufferScale.x=d.fb
    imgui.GetIO().DisplayFramebufferScale.y=d.fb
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
      local due=polling.due(force_refresh); force_refresh=false
      if due then state=mail.poll(state); path=state.path or path end
      if state.quit_for_restart then
        if companion then
          if not companion.restart_stop then
            Worker.command(companion,'stop',{keep=true}); companion.restart_stop=true
          end
          -- A helper that already failed needs no further stop; waiting for a
          -- 'stopped' phase that can never arrive strands :quit-for-restart.
          local settled=companion.phase=='stopped' or companion.phase=='failed'
          if not settled or not companion.exited then return end
          keeping_companion=true
        end
        SessionFile.save(state.project_root,Session.snapshot(state,chrome()))
        love.event.quit(0); return
      end
      if due and not state.waiting then
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
    if state.fit_pending then state=Events.settle(state,Display.dims(display())) end
    if probe then
      local passed,reason=pcall(probe,frames,state,{inspector=inspector,source_window=source_window,detail_window=detail_window,imgui=imgui,mail=mail,fonts=fonts,display=display,chrome=chrome})
      if not passed then io.stderr:write('Navigation acceptance failed: '..tostring(reason)..'\n'); love.event.quit(1); return end
    end
    imgui.love.Update(dt); imgui.NewFrame(); frame_active=true
    if test_seconds and elapsed>=test_seconds then love.event.quit(0) end
  end
  function love.draw()
    if not frame_active then return end
    local bg=T.colors.canvas_bg
    love.graphics.clear(bg[1]/255,bg[2]/255,bg[3]/255,1)
    local d=display()
    local height=d.window_h
    local zoom=Events.zoom_of(state)
    local scale=Display.canvas_scale(d,zoom)
    local canvas_w=d.canvas_px
    -- Both renderers consume selected/hover hits in the same display space.
    local view={selected=state.selected,hover=state.hover}
    local list=imgui.GetBackgroundDrawList_Nil()
    list:PushClipRect(imgui.ImVec2_Float(0,0),imgui.ImVec2_Float(canvas_w,height),true)
    Edges.draw(imgui,list,state.scene,-(state.cam_x or 0)*scale,-(state.cam_y or 0)*scale,scale,view,d.ui)
    renderer.draw(state.scene,d.window_w,height,{zoom=zoom,ui=d.ui,cam_x=state.cam_x or 0,cam_y=state.cam_y or 0,selected=state.selected,hover=state.hover})
    list:PopClipRect()
    inspector.draw(state,d)
    if detail_window then detail_window.draw(state,d) end
    Menu.draw(imgui,state,state.selected,function(op,extra) mail.request(op,extra) end)
    if source_window then source_window.draw() end
    list:PushClipRect(imgui.ImVec2_Float(0,0),imgui.ImVec2_Float(canvas_w,height),true)
    renderer.chrome(state,d.view_w,d.view_h,d.ui)
    if show_performance then
      list:AddText_FontPtr(fonts.body,Fonts.sizes.body*d.ui,imgui.ImVec2_Float(12*d.ui,48*d.ui),T.pack(T.colors.muted),
        string.format(Strings.performance,love.timer.getFPS(),frame_ms),nil,0,nil)
    end
    list:PopClipRect()
    if dragging_edge or Display.on_inspector_edge(d,love.mouse.getX()) then
      imgui.SetMouseCursor(imgui.ImGuiMouseCursor_ResizeEW)
    end
    imgui.Render(); frame_active=false; imgui.love.RenderDrawLists()
    if evidence and frames==4 then love.graphics.captureScreenshot(function(data) local f=assert(io.open(evidence..'/render.png','wb')); f:write(data:encode('png'):getString()); f:close() end) end
  end
  function love.quit()
    imgui.love.Shutdown()
    if companion and not companion.exited then
      Worker.command(companion,'stop',{keep=keeping_companion})
    end
  end
  function love.textinput(t) imgui.love.TextInput(t) end
  -- Events and their retained pointer use DPI-independent view units.
  local function dims()
    local d=Display.dims(display())
    d.control=love.keyboard.isDown('lctrl','rctrl'); d.shift=love.keyboard.isDown('lshift','rshift'); return d
  end
  local function on_canvas(x) return Display.in_canvas(display(),x) end
  local function view_xy(x,y) return Display.to_view(display(),x,y) end
  function love.keypressed(k)
    imgui.love.KeyPressed(k)
    if k=='f' and love.keyboard.isDown('lctrl','rctrl') then inspector.focus_search(); return end
    if not imgui.love.GetWantCaptureKeyboard() and not inspector.search_active() then
      if k=='f3' then show_performance=not show_performance end
      if k=='r' then force_refresh=true end
      state=Events.on_key(state,k,dims())
    end
  end
  function love.mousemoved(x,y)
    imgui.love.MouseMoved(x,y)
    if dragging_edge then inspector_w=Display.drag_inspector(display(),x); return end
    -- Active canvas drags retain ownership through sidebar/window crossings.
    if state.drag then state=Events.on_drag(state,view_xy(x,y))
    elseif not imgui.love.GetWantCaptureMouse() and on_canvas(x) then state=Events.on_move(state,view_xy(x,y))
    else state=Events.on_leave(state) end
  end
  function love.mousepressed(x,y,button,istouch,presses)
    imgui.love.MousePressed(button)
    -- The sidebar edge grip wins over the sidebar window that would capture it.
    if button==1 and Display.on_inspector_edge(display(),x) and not (imgui.IsPopupOpen_Str and imgui.IsPopupOpen_Str('',imgui.ImGuiPopupFlags_AnyPopupId or 0)) then
      dragging_edge=true; return
    end
    if imgui.love.GetWantCaptureMouse() or not on_canvas(x) then return end
    local vx,vy=view_xy(x,y)
    if button==2 then
      state=Events.on_press(state,vx,vy)
      if Events.element_menu(state.selected) then imgui.OpenPopup_Str('uml-element-mail') end
    elseif button==1 then
      -- Both transitions honour the back hotspot first.
      if (presses or 1)>=2 then state=Events.on_double_press(state,vx,vy) else state=Events.on_left_press(state,vx,vy) end
    end
  end
  function love.mousereleased(x,y,button)
    imgui.love.MouseReleased(button)
    if button==1 and dragging_edge then inspector_w=Display.drag_inspector(display(),x); dragging_edge=false end
    if button==1 and state.drag then state=Events.on_release(state,view_xy(x,y)) end
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
