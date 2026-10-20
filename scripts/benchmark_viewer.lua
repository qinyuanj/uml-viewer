-- Process-local benchmark driver. No desktop input or performance assertions.
-- Phases: warmup, idle, interaction (wheel/Ctrl zoom/P/hover) and detail (the
-- class card opened by a real left press on the rendered diagram).
local E=require('uml.edn')
local G=require('uml.domain.geom')
local Hit=require('uml.engine.hit')
local Display=require('uml.domain.display')
local Detail=require('uml.application.detail')
-- Detail.for_state compiles the whole document plus the target's parent view
-- (~80 ms for the example). Both the card adapter and the selector summary call
-- it, so the driver counts every call by calling module: card_builds is the
-- per-frame rebuild that made the open card single-digit FPS.
local detail_calls=0
local detail_build_ms=0
local detail_by_module={}
local detail_ms_by_module={}
local function module_of(src)
  src=src or '?'
  if src:find('detail_window',1,true) then return 'detail_window' end
  if src:find('inspector',1,true) then return 'inspector' end
  return 'other'
end
-- Both consumers now build through application/detail_cache, so the immediate
-- caller is the cache, not the adapter that wanted the model. Walk up to the
-- first frame that is not the cache to keep the per-consumer attribution.
-- Levels are relative to this helper: 1 is this frame, 2 the counting wrapper,
-- 3 the cache closure, so the walk starts at 3.
local function calling_module()
  for level=3,9 do
    local info=debug.getinfo(level,'S')
    if not info then break end
    local src=info.short_src or ''
    if not src:find('detail_cache',1,true) then return module_of(src) end
  end
  return 'other'
end
do
  local build=Detail.for_state
  Detail.for_state=function(...)
    detail_calls=detail_calls+1
    local module=calling_module()
    detail_by_module[module]=(detail_by_module[module] or 0)+1
    local started=love.timer.getTime()
    local result=build(...)
    local cost=(love.timer.getTime()-started)*1000
    detail_build_ms=detail_build_ms+cost
    detail_ms_by_module[module]=(detail_ms_by_module[module] or 0)+cost
    return result
  end
end
-- Warmup is fixed at 2s (init plus the first fit); the rest of the budget is
-- split evenly between idle, interaction and detail, leaving 1s of slack before
-- the process's own UML_VIEWER_TEST_SECONDS quit. A 16s budget gives the usual
-- 2/4.3/4.3/4.3 split; a wide synthetic document that renders near 1 fps is
-- given a longer budget by the runner so every phase still produces frames.
local seconds=tonumber(os.getenv('UML_VIEWER_TEST_SECONDS')) or 16
local share=math.max(1,(seconds-3)/3)
local schedule={{'warmup',2},{'idle',share},{'interaction',share},{'detail',share}}
local PHASES={'idle','interaction','detail','detail_card'}
local samples,counters={},{}
for _,name in ipairs(PHASES) do samples[name]={}; counters[name]={} end
local started,finished,last,metadata
local prev_calls,prev_card,prev_module=0,0,0
local interaction_frame,detail_step=0,0
local target_id,target_click,target_ns,opened_id,opened_frame,open_ms,pending,attempts
local attempt_log={}
-- 15 frames is above any single-digit sample while staying reachable for the
-- synthetic document, whose phases run at under 1.5 fps. The point is to catch
-- a starved or missing phase, not to assert a frame budget.
local MIN_FRAMES=15
local function phase_at(age)
  local edge=0
  for _,entry in ipairs(schedule) do edge=edge+entry[2]; if age<edge then return entry[1] end end
  return 'done'
end
local function with_control(fn)
  local is_down=love.keyboard.isDown
  love.keyboard.isDown=function(...) if select(1,...)=='lctrl' then return true end; return is_down(...) end
  fn(); love.keyboard.isDown=is_down
end
-- Where a single click (presses>=2 for a box, 1 for a child row) can open the
-- card. Each point is confirmed through the same hit test the LOVE callback
-- runs, so nothing here fabricates a state the click would not produce.
local function scene_points(scene,names)
  local points={}
  local function put(id,x,y,presses,parent)
    local key=E.encode(id)
    if id~=nil and not points[key] then
      points[key]={id=id,ns=names[G.name(id)],x=x,y=y,presses=presses,parent=parent}
    end
  end
  for _,c in ipairs(G.get(scene,'classes') or {}) do
    local r=G.get(c,'rect'); local id=G.get(c,'id'); local parent=G.name(id)
    local drillable=G.get(c,'drill?')==true
    if r and id then
      local x=G.cx(r); local top=G.get(r,'y'); local y0=math.min(top+2,G.bottom(r)-1)
      local foreign=G.get(c,'foreign')==true
      if not drillable and not G.get(c,'dummy?') and not foreign then
        local hit=Hit.at(scene,x,y0)
        if hit and G.name(G.get(hit,'kind'))=='class' and E.equal(id,G.get(hit,'id')) and G.get(hit,'drill?')~=true then
          put(id,x,y0,2,parent)
        end
      end
      -- Mirror the compiled box's line pitch to land on a child row. Collapsed
      -- namespace boxes hide their members, so this fires on views that show
      -- contents.
      local ly=top+16
      for _,line in ipairs(G.get(c,'lines') or {}) do
        local kind=G.name(G.get(line,'kind'))
        if kind=='rule' then ly=ly+12
        else
          if kind=='child' then
            local lid=G.get(line,'id'); local y=math.min(ly+8,G.bottom(r)-1)
            local hit=Hit.at(scene,x,y)
            if hit and G.name(G.get(hit,'kind'))=='child' and E.equal(lid,G.get(hit,'id')) then
              put(lid,x,y,1,parent)
            end
          end
          ly=ly+18
        end
      end
    end
  end
  local drills={}
  for _,c in ipairs(G.get(scene,'classes') or {}) do
    local r=G.get(c,'rect'); local id=G.get(c,'id')
    if r and id and G.get(c,'drill?')==true and G.get(c,'foreign')~=true and not G.get(c,'dummy?') then
      drills[#drills+1]={id=id,name=G.name(id),x=G.cx(r),y=G.get(r,'y')+2}
    end
  end
  table.sort(drills,function(a,b) return a.name<b.name end)
  return points,drills
end
-- Authored identities ordered by :ns then id, plus a scene-name -> :ns map for
-- labels the compiled scene reuses.
local function candidates(document)
  local list,seen,names={},{},{}
  local function add(c)
    local id=G.get(c,'id'); local ns=G.get(c,'ns')
    if id~=nil then
      local key=E.encode(id); names[G.name(id)]=ns or names[G.name(id)]
      if ns and not seen[key] then seen[key]=true; list[#list+1]={id=id,ns=ns,key=key} end
    end
    for _,child in ipairs(G.get(c,'contents') or {}) do add(child) end
  end
  for _,c in ipairs(G.get(document,'classes') or {}) do add(c) end
  for _,p in ipairs(G.get(document,'packages') or {}) do for _,c in ipairs(G.get(p,'classes') or {}) do add(c) end end
  table.sort(list,function(a,b) if a.ns~=b.ns then return a.ns<b.ns end; return a.key<b.key end)
  return list,names
end
-- Deterministic target for the current view, in three tiers so the example and
-- the flat synthetic document both end up on a real card:
--   1. an on-screen class whose id carries a namespace path (`uml.graph`); the
--      card Detail.for_state builds for it compiles both the root view and the
--      class's own parent view, the expensive case;
--   2. otherwise drill into the alphabetically first namespace box that is on
--      screen (the same double press a user makes) and retry next frame;
--   3. otherwise the on-screen class whose :ns sorts first, i.e. the flat
--      synthetic document's earliest authored box.
-- Only points whose window position is inside the canvas count: a box the fit
-- pushed off screen cannot be clicked by a user either. A child row, when the
-- view renders one, is the same card reached with a single press and wins.
local function resolve(state,display)
  if not state.scene or not state.document then return end
  local list,names=candidates(state.document)
  local points,drills=scene_points(state.scene,names)
  local view={zoom=state.zoom or 1,cam_x=state.cam_x or 0,cam_y=state.cam_y or 0}
  local function on_screen(p)
    local sx,sy=Display.world_to_window(display,view,p.x,p.y)
    if sx>=0 and sx<display.canvas_px and sy>=0 and sy<display.window_h then
      p.sx,p.sy=sx,sy; return true
    end
    return false
  end
  local rows,dotted={},{}
  for key,p in pairs(points) do
    if on_screen(p) then
      if p.presses==1 then rows[#rows+1]={key=key,p=p}
      else
        rows[#rows+1]={key=key,p=p}
        if G.name(p.id):find('%.') then dotted[#dotted+1]={key=key,p=p} end
      end
    end
  end
  local function by_ns(a,b)
    local an,bn=a.p.ns or '~',b.p.ns or '~'
    if an~=bn then return an<bn end
    return a.key<b.key
  end
  table.sort(dotted,by_ns)
  if #dotted>0 then return {kind='click',id=dotted[1].p.id,ns=dotted[1].p.ns,p=dotted[1].p} end
  table.sort(drills,function(a,b) return a.name<b.name end)
  for _,d in ipairs(drills) do
    if on_screen(d) then return {kind='drill',id=d.id,p=d} end
  end
  table.sort(rows,by_ns)
  if #rows>0 then return {kind='click',id=rows[1].p.id,ns=rows[1].p.ns,p=rows[1].p} end
end
local function phase_stats(values,c)
  c=c or {}
  local out={frames=#values,card_builds=c.card or 0,builds=c.builds or 0,
    detail_visible_frames=c.visible or 0}
  if c.min then out.zoom_min=c.min end
  if c.max then out.zoom_max=c.max end
  if #values>0 then
    table.sort(values); local total=0; for _,v in ipairs(values) do total=total+v end
    out.fps=1000/(total/#values); out.mean_ms=total/#values
    out.median_ms=values[math.ceil(#values*.5)]; out.p95_ms=values[math.ceil(#values*.95)]
    out.card_builds_per_frame=(c.card or 0)/#values
    out.builds_per_frame=(c.builds or 0)/#values
  end
  return out
end
local function enved(v)
  local kind=E.kind(v)
  if kind~='table' then return v end -- already an EDN value
  if #v>0 and next(v,#v+1)==nil then
    local out=E.vector(); for i,x in ipairs(v) do out[i]=enved(x) end; return out
  end
  local out=E.map(); for k,x in pairs(v) do E.assoc(out,E.keyword(tostring(k)),enved(x)) end; return out
end
local function write(text,path)
  local f=assert(io.open(os.getenv('UML_VIEWER_EVIDENCE')..'/'..path,'wb')); f:write(text); f:close()
end
local function fmt_phase(name,p)
  return string.format('phase=%s frames=%d fps=%.3f mean_ms=%.3f median_ms=%.3f p95_ms=%.3f card_builds=%d card_builds_per_frame=%.3f builds=%d visible_frames=%d',
    name,p.frames or 0,p.fps or 0,p.mean_ms or 0,p.median_ms or 0,p.p95_ms or 0,
    p.card_builds or 0,p.card_builds_per_frame or 0,p.builds or 0,p.detail_visible_frames or 0)
end
local function summary(phases,detail)
  local out={}
  for _,name in ipairs(PHASES) do out[#out+1]=fmt_phase(name,phases[name] or {frames=0}) end
  out[#out+1]=string.format('detail opened=%s id=%s ns=%s click=%s visible_frames=%d first_card_frame_ms=%.3f',
    tostring(detail.opened),tostring(detail.id),tostring(detail.ns),tostring(detail.click),
    detail.visible or 0,detail['first-card-frame-ms'] or 0)
  out[#out+1]=string.format('detail_calls=%d calls_mean_ms=%.3f card_builds=%d card_builds_mean_ms=%.3f inspector_builds=%d inspector_builds_mean_ms=%.3f',
    detail.calls or 0,(detail['calls'] or 0)>0 and (detail['build-ms'] or 0)/(detail.calls or 1) or 0,
    detail['card-builds'] or 0,(detail['card-builds'] or 0)>0 and (detail['card-build-ms'] or 0)/(detail['card-builds'] or 1) or 0,
    detail['inspector-builds'] or 0,(detail['inspector-builds'] or 0)>0 and (detail['inspector-build-ms'] or 0)/(detail['inspector-builds'] or 1) or 0)
  return table.concat(out,'\n')..'\n'
end
-- A starved phase, a card that never opened, or an unattributed build source is
-- a failed benchmark: it must never read as a healthy run.
local function failures(phases,detail)
  local out={}
  for _,name in ipairs{'idle','interaction','detail'} do
    local p=phases[name] or {frames=0}
    if p.frames<MIN_FRAMES then
      out[#out+1]=string.format('%s produced %d frames (minimum %d)',name,p.frames,MIN_FRAMES)
    end
  end
  if not detail.opened then out[#out+1]='the detail card never opened from the pressed box' end
  if (detail.visible or 0)<MIN_FRAMES then
    out[#out+1]=string.format('the detail card was visible for %d frames (minimum %d)',detail.visible or 0,MIN_FRAMES)
  end
  if (detail['card-builds'] or 0)==0 then
    out[#out+1]='no Detail.for_state call came from the detail window; the card was measured without building'
  end
  return out
end
return function(frame,state,a)
  local now=love.timer.getTime()
  started=started or now
  local age=now-started
  local phase=phase_at(age)
  -- Attribute the calls made since the previous probe to the module that made
  -- them; this is what a per-frame rebuild would show as card_builds_per_frame.
  local card=detail_by_module.detail_window or 0
  local builds=detail_calls-prev_calls; prev_calls=detail_calls
  local made_card=card-prev_card; prev_card=card
  local visible=a.detail_window and a.detail_window.visible(state) or false
  -- The probe runs before this frame's draw: the first frame that reports a
  -- detail target is the one whose interval covers the card's first paint.
  local opening=visible and opened_id==nil
  if opening then opened_id=E.encode(state.detail_id); opened_frame=frame end
  if last and samples[phase] then
    local elapsed=(now-last)*1000
    local c=counters[phase]; local z=state.zoom or 1
    c.min=c.min and math.min(c.min,z) or z; c.max=c.max and math.max(c.max,z) or z
    c.builds=(c.builds or 0)+builds; c.card=(c.card or 0)+made_card
    if visible then c.visible=(c.visible or 0)+1 end
    samples[phase][#samples[phase]+1]=elapsed
    if opening then open_ms=elapsed end
  end
  if last and phase=='detail' and visible and not opening then
    -- Intervals that covered an already-open card, kept apart from the opening
    -- frame so the card's own steady state is readable.
    local c=counters.detail_card
    c.builds=(c.builds or 0)+builds; c.card=(c.card or 0)+made_card
    c.visible=(c.visible or 0)+1
    samples.detail_card[#samples.detail_card+1]=(now-last)*1000
  end
  last=now
  if not metadata then
    local w,h,flags=love.window.getMode()
    local major,minor,revision=love.getVersion()
    local renderer,version,vendor,device=love.graphics.getRendererInfo()
    local d=a.display()
    metadata={love=major..'.'..minor..'.'..revision,luajit=jit and jit.version or _VERSION,
      window_w=w,window_h=h,highdpi=flags.highdpi,vsync=flags.vsync,
      dpi=love.window.getDPIScale(),ui=d.ui,canvas_px=d.canvas_px,zoom=state.zoom or 1,
      source_classes=#(G.get(state.document,'classes') or {}),source_edges=#(G.get(state.document,'edges') or {}),
      visible_classes=#(G.get(state.scene,'classes') or {}),visible_edges=#(G.get(state.scene,'edges') or {}),
      renderer=renderer,renderer_version=version,vendor=vendor,device=device}
    love.keypressed('f3')
  end
  if phase=='interaction' then
    -- Repeat a representative navigation cycle through actual LOVE callbacks.
    local step=interaction_frame%8; interaction_frame=interaction_frame+1
    if step==0 then love.wheelmoved(0,1)
    elseif step==1 then love.wheelmoved(0,-1)
    elseif step==2 or step==3 then
      with_control(function() love.keypressed(step==2 and '=' or '-') end)
    elseif step==4 then love.mousemoved(100,100)
    elseif step==5 then love.keypressed('p') end
  end
  if phase=='detail' then
    detail_step=detail_step+1
    if detail_step==1 then
      -- Return to the namespace tree and fit the canvas so every box is inside
      -- the clickable canvas region.
      if G.get(state.extra,'proposal') or G.get(state.extra,'proposal-id') then love.keypressed('p') end
      with_control(function() love.keypressed('0') end)
    elseif pending then
      love.mousepressed(pending.x,pending.y,1,false,pending.presses)
      love.mousereleased(pending.x,pending.y,1)
      target_click=pending.presses>=2 and 'box' or 'child'
      assert(#attempt_log<40)
      attempt_log[#attempt_log+1]=string.format('step=%d press=%.1f,%.1f n=%d detail_id=%s',detail_step,pending.x,pending.y,pending.presses,tostring(state.detail_id and E.encode(state.detail_id)))
      pending=nil
    elseif opened_id==nil and detail_step%2==0 and (attempts or 0)<8 then
      -- Retry every other frame until the card is open; drilling into a
      -- namespace box lands a view whose boxes open the expensive card.
      attempts=(attempts or 0)+1
      local target=resolve(state,a.display())
      if target then
        local sx,sy=target.p.sx,target.p.sy
        local d=a.display()
        local capture=(function() local ok,value=pcall(function() return a.imgui.love.GetWantCaptureMouse() end); return ok and tostring(value) or '?' end)()
        if target.kind=='click' then
          -- Park the pointer over the canvas one frame early so imgui's capture
          -- state matches the press, as it would for a moving user.
          love.mousemoved(sx,sy)
          pending={x=sx,y=sy,presses=target.p.presses}
          target_id,target_ns=E.encode(target.id),target.ns
          assert(#attempt_log<40)
          attempt_log[#attempt_log+1]=string.format('step=%d aim=%s at=%.1f,%.1f zoom=%.4f cam=%.1f,%.1f capture=%s canvas=%d detail=%s',detail_step,E.encode(target.id),sx,sy,state.zoom or 1,state.cam_x or 0,state.cam_y or 0,capture,d.canvas_px,tostring(state.detail_id and E.encode(state.detail_id)))
        elseif target.kind=='drill' then
          love.mousemoved(sx,sy)
          love.mousepressed(sx,sy,1,false,2)
          love.mousereleased(sx,sy,1)
          assert(#attempt_log<40)
          attempt_log[#attempt_log+1]=string.format('step=%d drill=%s at=%.1f,%.1f capture=%s canvas=%d selected=%s',detail_step,E.encode(target.id),sx,sy,capture,d.canvas_px,E.encode(state.selected))
        end
      else
        assert(#attempt_log<40)
        attempt_log[#attempt_log+1]=string.format('step=%d no-target',detail_step)
      end
    end
  end
  if phase=='done' and not finished then
    finished=true
    local phases={}
    for _,name in ipairs(PHASES) do phases[name]=phase_stats(samples[name],counters[name]) end
    local visible_frames=counters.detail.visible or 0
    local detail={opened=opened_id~=nil,id=opened_id or target_id or 'none',
      ns=target_ns or 'unknown',click=target_click or 'none',visible=visible_frames,
      ['calls']=detail_calls,['card-builds']=detail_by_module.detail_window or 0,
      ['inspector-builds']=detail_by_module.inspector or 0,
      ['other-builds']=detail_by_module.other or 0,
      ['build-ms']=detail_build_ms,
      ['card-build-ms']=detail_ms_by_module.detail_window or 0,
      ['inspector-build-ms']=detail_ms_by_module.inspector or 0,
      ['opened-at-frame']=opened_frame or 0,['first-card-frame-ms']=open_ms or 0,
      ['total-frames']=frame,attempts=E.vector(attempt_log)}
    if metadata then
      metadata.detail_id=detail.id; metadata.detail_ns=detail.ns
      metadata.detail_click=detail.click; metadata.detail_opened=detail.opened
      metadata.detail_calls=detail.calls
      metadata.detail_card_builds=detail['card-builds']
      metadata.detail_inspector_builds=detail['inspector-builds']
      metadata.detail_visible_frames=visible_frames
    end
    local problems=failures(phases,detail)
    local schedule_text={}
    for _,entry in ipairs(schedule) do schedule_text[#schedule_text+1]=string.format('%.1fs %s',entry[2],entry[1]) end
    local result={environment=metadata,
      method=string.format('%s (from UML_VIEWER_TEST_SECONDS=%s); %s; vsync unchanged; frame deltas include polling and render/present; card_builds counts Detail.for_state calls from the detail window (0 per frame = cached, >0 = a per-frame rebuild), inspector_builds the selector summary; detail_card keeps intervals that covered an already-open card',
        table.concat(schedule_text,', '),tostring(seconds),
        'detail phase fits the canvas, then opens the card through LOVE mouse callbacks: a single press on a child row, else a double press on a class box, else a double press into the first namespace box to reach one'),
      detail=detail,phases=phases,failures=E.vector()}
    for _,problem in ipairs(problems) do result.failures[#result.failures+1]=problem end
    write(E.encode(enved(result)),'benchmark.edn')
    local text=summary(phases,detail)
    if #problems>0 then text=text..'FAILED: '..table.concat(problems,'; ')..'\n' end
    write(text,'summary.txt')
    io.stdout:write(text); io.stdout:flush()
    if #problems>0 then
      io.stderr:write('Benchmark failed: '..table.concat(problems,'; ')..'\n')
      love.event.quit(1); return
    end
    love.event.quit(0)
  end
end
