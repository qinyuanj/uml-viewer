-- LOVE reference run of the viewer stack benchmark protocol (bench/README.md).
-- Loaded through UML_VIEWER_TEST_DRIVER; writes protocol.edn and summary.txt to
-- UML_VIEWER_EVIDENCE. The camera is set directly each frame from phase time,
-- so every candidate follows the same trajectory whatever its frame rate.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local Hit=require('uml.engine.hit')

-- Phase length: at least MIN_S seconds and MIN_FRAMES frames, at most MAX_S.
local WARMUP_S,MIN_S,MIN_FRAMES,MAX_S=3,10,20,180
-- UML_VIEWER_LEGACY=1 restricts the run to the phases the current stack can
-- survive: the cimgui 16-bit vertex ceiling (no culling; arc and glyph counts
-- grow with zoom) aborts the process on detail-zoom phases and on any
-- realistic scene at fit zoom, so only chain-500 yields numbers at all.
local LEGACY=os.getenv('UML_VIEWER_LEGACY')=='1'
local PHASES=LEGACY and {'idle','pan-fit'} or {'idle','pan-detail','zoom-detail','pan-fit','zoom-fit'}
local TAU=2*math.pi

local function clk_tck()
  local p=io.popen('getconf CLK_TCK'); local v=p and tonumber(p:read('*l')); if p then p:close() end; return v or 100
end
local TCK=clk_tck()
-- utime+stime of this process in seconds (fields 14 and 15 of /proc/self/stat).
local function cpu_s()
  local f=io.open('/proc/self/stat'); if not f then return nil end
  local s=f:read('*l'); f:close()
  local rest=s:match('%) (.*)$'); local fields={}
  for x in rest:gmatch('%S+') do fields[#fields+1]=x end
  -- rest starts at field 3 (state), so utime/stime are rest[12]/rest[13].
  return (tonumber(fields[12])+tonumber(fields[13]))/TCK
end

local started,last,phase_i,phase_t0,phase_cpu0,metadata,geometry,vtx_wrapped
local samples={}
local results={}

-- Camera targets in world units. view_w/view_h are the canvas in view units.
local function camera(state,cx,cy,zoom,view)
  state.zoom=zoom
  state.cam_x=cx-view.w/(2*zoom); state.cam_y=cy-view.h/(2*zoom)
end
local function trajectory(name,t,geo)
  local cx,cy=geo.cx,geo.cy
  if name=='idle' then return nil
  elseif name=='pan-detail' then
    -- Circle of radius 1200 world px, 8 s period (about 940 px/s at zoom 1).
    return cx+1200*math.cos(TAU*t/8),cy+1200*math.sin(TAU*t/8),1
  elseif name=='zoom-detail' then
    -- Geometric 0.5x <-> 2x around the scene centre, 4 s period.
    local u=.5-.5*math.cos(TAU*t/4)
    return cx,cy,math.exp(math.log(.5)+u*(math.log(2)-math.log(.5)))
  elseif name=='pan-fit' then
    -- Whole diagram in view; Lissajous over 25% of the scene, 8 s period.
    return cx+.25*geo.w*math.sin(TAU*t/8),cy+.25*geo.h*math.sin(2*TAU*t/8),geo.fit
  elseif name=='zoom-fit' then
    -- fit <-> 4x fit: crosses the overview LOD bands, 4 s period.
    local u=.5-.5*math.cos(TAU*t/4)
    return cx,cy,geo.fit*math.exp(u*math.log(geo['zoom-hi']/geo.fit))
  end
end

local function stats(name,values,cpu,wall,zmin,zmax)
  local out={phase=name,frames=#values,['wall-s']=wall,['cpu-pct']=cpu and wall>0 and 100*cpu/wall or E.nil_value,
    ['zoom-min']=zmin or E.nil_value,['zoom-max']=zmax or E.nil_value}
  if #values>0 then
    table.sort(values); local total=0; for _,v in ipairs(values) do total=total+v end
    local function q(p) return values[math.max(1,math.ceil(#values*p))] end
    out.fps=1000*#values/total; out['mean-ms']=total/#values
    out['median-ms']=q(.5); out['p95-ms']=q(.95); out['p99-ms']=q(.99); out['max-ms']=values[#values]
  end
  return out
end
local function enved(v)
  if type(v)~='table' or E.kind(v)~='table' then return v end
  if #v>0 then local out=E.vector(); for i,x in ipairs(v) do out[i]=enved(x) end; return out end
  local out=E.map(); for k,x in pairs(v) do E.assoc(out,E.keyword(k),enved(x)) end; return out
end
local function write(name,text)
  local f=assert(io.open(os.getenv('UML_VIEWER_EVIDENCE')..'/'..name,'wb')); f:write(text); f:close()
end
local function fmt(x) return type(x)=='number' and string.format('%.2f',x) or '-' end

return function(frame,state,a)
  -- Debug: UML_VIEWER_DEBUG_VTX=1 prints per-list vertex counts before Render.
  if os.getenv('UML_VIEWER_DEBUG_VTX')=='1' and not vtx_wrapped then
    vtx_wrapped=true
    local R=a.imgui.Render
    a.imgui.Render=function(...)
      local l=a.imgui.GetBackgroundDrawList_Nil()
      io.stderr:write(string.format('VTX bg=%d idx=%d\n',l.VtxBuffer.Size,l.IdxBuffer.Size))
      return R(...)
    end
  end
  local now=love.timer.getTime()
  local d=a.display()
  local view={w=d.view_w,h=d.view_h}
  if not geometry then
    local size=G.get(state.scene,'size'); local x0,y0=G.get(size,'min-x') or 0,G.get(size,'min-y') or 0
    local w,h=G.get(size,'w')-x0,G.get(size,'h')-y0
    -- Unclamped fit: the protocol needs the whole diagram even where the
    -- LOVE viewer's own 0.1 zoom floor stops a user short of it.
    local fit=math.min(1,view.w/w,view.h/h)
    if LEGACY then
      -- The viewer clamps zoom to >=0.1, and that clamp is what keeps the
      -- chain scene renderable: text is clip-culled, geometry is not, so the
      -- true fit would draw every glyph (74k vertices, over the 64k ceiling).
      fit=math.max(.1,fit)
    end
    geometry={cx=x0+w/2,cy=y0+h/2,w=w,h=h,fit=fit,['zoom-hi']=fit*(LEGACY and 1.3 or 4)}
    -- UML_VIEWER_HIDE_EDGES=1 renders the scene through the viewer's own
    -- declutter-triangles mode (compose/hierarchy): edges become dependency
    -- triangles. Geometry is unchanged, so the trajectory still applies. The
    -- direct-edge runs abort in cimgui's 16-bit ImDrawIdx assert on these
    -- scenes; this mode is the measurable baseline variant, and the crash
    -- itself is a recorded baseline result.
    if os.getenv('UML_VIEWER_HIDE_EDGES')=='1' then
      G.put(G.get(state.scene,'diagram'),'hide-edges',true)
      G.put(state.scene,'dep-indicators',Hit.dep_indicators(state.scene))
      for _,e in ipairs(G.get(state.scene,'edges') or {}) do G.put(e,'strokes',E.vector()) end
    end
    local renderer,version,vendor,device=love.graphics.getRendererInfo()
    local ww,wh,flags=love.window.getMode()
    local major,minor,rev=love.getVersion()
    metadata={stack='love',love=major..'.'..minor..'.'..rev,runtime=jit and jit.version or _VERSION,
      renderer=renderer,['renderer-version']=version,vendor=vendor,device=device,
      ['gallium-driver']=os.getenv('GALLIUM_DRIVER') or '',
      ['window-w']=ww,['window-h']=wh,vsync=flags.vsync,['refresh-hz']=flags.refreshrate,
      dpi=love.window.getDPIScale(),ui=d.ui,['canvas-px']=d.canvas_px,['view-w']=view.w,['view-h']=view.h,
      ['scene-w']=w,['scene-h']=h,['fit-zoom']=geometry.fit,
      classes=#(G.get(state.scene,'classes') or {}),edges=#(G.get(state.scene,'edges') or {}),
      packages=#(G.get(state.scene,'packages') or {}),['clk-tck']=TCK,
      ['hide-edges']=G.get(G.get(state.scene,'diagram'),'hide-edges')==true,legacy=LEGACY}
    started=now
  end
  local age=now-started
  if age<WARMUP_S then
    camera(state,geometry.cx,geometry.cy,LEGACY and geometry.fit or 1,view); last=now; return
  end
  if not phase_i then phase_i=1; phase_t0=now; phase_cpu0=cpu_s(); samples={} end
  local name=PHASES[phase_i]
  -- The interval since the last probe covers the previous frame's draw.
  if last then samples[#samples+1]=(now-last)*1000 end
  last=now
  local t=now-phase_t0
  local cx,cy,z=trajectory(name,t,geometry)
  if cx then
    camera(state,cx,cy,z,view)
    samples.zmin=math.min(samples.zmin or z,z); samples.zmax=math.max(samples.zmax or z,z)
  end
  if (t>=MIN_S and #samples>=MIN_FRAMES) or t>=MAX_S then
    local cpu1=cpu_s()
    results[#results+1]=stats(name,samples,phase_cpu0 and cpu1 and cpu1-phase_cpu0,t,samples.zmin,samples.zmax)
    phase_i=phase_i+1; phase_t0=now; phase_cpu0=cpu1; samples={}
    if phase_i>#PHASES then
      local lines={}
      for _,r in ipairs(results) do
        lines[#lines+1]=string.format('phase=%s frames=%d fps=%s mean_ms=%s median_ms=%s p95_ms=%s p99_ms=%s max_ms=%s cpu_pct=%s zoom=%s..%s',
          r.phase,r.frames,fmt(r.fps),fmt(r['mean-ms']),fmt(r['median-ms']),fmt(r['p95-ms']),fmt(r['p99-ms']),fmt(r['max-ms']),
          fmt(r['cpu-pct']),fmt(r['zoom-min']),fmt(r['zoom-max']))
      end
      local text=string.format('renderer=%s | %s gallium=%s fit_zoom=%.4f\n',metadata.renderer,metadata.device,metadata['gallium-driver'],geometry.fit)..table.concat(lines,'\n')..'\n'
      write('protocol.edn',E.encode(enved({protocol=1,environment=metadata,phases=results}))..'\n')
      write('summary.txt',text); io.stdout:write(text); io.stdout:flush()
      love.event.quit(0)
    end
  end
end
