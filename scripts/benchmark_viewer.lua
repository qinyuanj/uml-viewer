-- Process-local benchmark driver. No desktop input or performance assertions.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local samples={idle={},interaction={}}
local started,finished,last,metadata
local interaction_frame=0
local ranges={idle={},interaction={}}
return function(frame,state,a)
  local now=love.timer.getTime()
  started=started or now
  local age=now-started
  local phase=age<2 and 'warmup' or age<7 and 'idle' or age<12 and 'interaction' or 'done'
  if last and samples[phase] then
    samples[phase][#samples[phase]+1]=(now-last)*1000
    local r=ranges[phase]; local z=state.zoom or 1
    r.min=r.min and math.min(r.min,z) or z; r.max=r.max and math.max(r.max,z) or z
  end
  last=now
  if not metadata then
    local w,h,flags=love.window.getMode()
    local major,minor,revision=love.getVersion()
    local renderer,version,vendor,device=love.graphics.getRendererInfo()
    metadata={love=major..'.'..minor..'.'..revision,luajit=jit and jit.version or _VERSION,
      window_w=w,window_h=h,highdpi=flags.highdpi,vsync=flags.vsync,
      dpi=love.window.getDPIScale(),ui=a.display().ui,zoom=state.zoom or 1,
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
      local is_down=love.keyboard.isDown
      love.keyboard.isDown=function(...) if select(1,...)=='lctrl' then return true end; return is_down(...) end
      love.keypressed(step==2 and '=' or '-')
      love.keyboard.isDown=is_down
    elseif step==4 then love.mousemoved(100,100)
    elseif step==5 then love.keypressed('p') end
  end
  if phase=='done' and not finished then
    finished=true
    local result={environment=metadata,method='2s warmup, 5s idle, 5s process-local wheel pan/Ctrl zoom/P/hover; vsync unchanged; frame deltas include polling and render/present',phases={}}
    for name,values in pairs(samples) do
      table.sort(values); local total=0; for _,v in ipairs(values) do total=total+v end
      result.phases[name]={frames=#values,fps=1000/(total/#values),mean_ms=total/#values,
        median_ms=values[math.ceil(#values*.5)],p95_ms=values[math.ceil(#values*.95)],
        zoom_min=ranges[name].min,zoom_max=ranges[name].max}
    end
    local f=assert(io.open(os.getenv('UML_VIEWER_EVIDENCE')..'/benchmark.edn','wb'))
    -- Convert ordinary Lua records at this evidence boundary.
    local function edn(v)
      if type(v)~='table' then return v end
      local out=E.map(); for k,x in pairs(v) do E.assoc(out,E.keyword(k),edn(x)) end; return out
    end
    f:write(E.encode(edn(result))); f:close(); love.event.quit(0)
  end
end
