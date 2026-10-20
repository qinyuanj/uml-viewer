local E=require('uml.edn')
local G=require('uml.domain.geom')
local M={}
local C=require('uml.domain.config')
local L=require('uml.engine.layout')
local S=require('uml.application.strings')
local F=require('uml.adapters.fonts')
local T=require('uml.domain.theme')
-- Text keeps the layout's 12px side padding; package titles also clear the C/M dots.
local pad,package_title_reserve=12,48
local function bands(c)
  return G.name(C.crap_band(C.crap_risk(G.get(c,'crap')))),G.name(C.mutation_band(C.mutation_ratio(c)))
end
-- Missing means either metric is absent (no CRAP score, or no mutation ratio).
-- It is decided from the nil metrics, never inferred from a red band: a
-- measured class can be red too. Missing stays red (each absent metric is
-- grade 1 under config), and adds the dashed border and 无数据 marker.
local function crap_grade(c) return C.crap_grade(C.crap_risk(G.get(c,'crap'))) end
local function mutation_grade(c) return C.mutation_grade(C.mutation_ratio(c)) end
local function missing(c) return C.crap_risk(G.get(c,'crap'))==nil or C.mutation_ratio(c)==nil end
-- The 4e69e34 box grade: the average of the CRAP and mutation 1-10 grades.
local function grade(c) return C.combined_grade(crap_grade(c),mutation_grade(c)) end
local function quality(commands,c,owner)
  local r=G.get(c,'rect'); local cb,mb=bands(c)
  for i,b in ipairs{cb,mb} do commands[#commands+1]={kind='quality',owner=owner,id=G.get(c,'id'),rect=r,text=i==1 and 'C' or 'M',band=b,grade=i==1 and crap_grade(c) or mutation_grade(c),radius=6,x=G.right(r)-24+(i-1)*14,y=G.get(r,'y')+10} end
end
-- The corner band holds the level (top-left) and the C/M dots and α/I marker
-- (top-right); text whose slot reaches into it must stop short of the badges.
local band_bottom,dots_reserve,marker_reserve=16,32,46
-- Vertical slots for a class's text lines. The layout keeps its golden 18px
-- line pitch, so the 18px names and members are packed into each
-- run of lines between rules (and the corner band / box bottom): nominal sizes
-- when they fit, scaled down together when they do not, with even gaps.
-- An unmeasured class reserves a bottom strip for its 无数据 marker.
M.no_data_strip=12
function M.text_slots(c,r,marker,unmeasured)
  local x,ry,w,h=G.get(r,'x'),G.get(r,'y'),G.get(r,'w'),G.get(r,'h')
  local out,run={},{}
  local y=ry+16; local run_top=ry+(G.get(c,'level')~=nil and 14 or 4)
  local stereo=G.name(G.get(c,'stereotype'))
  local function flush(run_bottom)
    local total=0; for _,e in ipairs(run) do total=total+e.size end
    if #run>0 then
      local avail=run_bottom-run_top; local k=math.min(1,avail/total); local gap=(avail-total*k)/(#run+1); local top=run_top+gap
      for _,e in ipairs(run) do
        e.top=top; e.h=e.size*k; top=top+e.h+gap
        e.max_w=w-2*pad+(e.top<ry+band_bottom and pad-(marker and marker_reserve or dots_reserve) or 0)
      end
    end
    run={}
  end
  for _,line in ipairs(G.get(c,'lines') or {}) do
    local kind=G.name(G.get(line,'kind'))
    if kind=='rule' then
      flush(y-5); out[#out+1]={kind='rule',x=x,y=y-4,w=w}; run_top=y-3; y=y+12
    else
      local italic=kind=='name' and ((stereo~=nil and stereo~='class') or G.get(c,'drill?')==true or #(G.get(c,'contents') or {})>0)
      local e={kind='text',role=kind,italic=italic,bold=kind=='name',center=kind=='name',box_w=w,size=kind=='name' and F.sizes.title or F.sizes.body,x=x+pad,y=y,text=G.get(line,'text') or ''}
      out[#out+1]=e; run[#run+1]=e; y=y+18
    end
  end
  flush(ry+h-4-(unmeasured and M.no_data_strip or 0))
  return out
end-- Adapter contract: a renderer consumes commands; no UI binding leaks into scenes.
-- view.selected / view.hover are engine hit maps (selected gold, hovered ink).
-- Colors are theme RGB triples; `band`/`red` keep the config traffic light.
function M.commands(scene,view)
  view=view or {}
  local sel_id=G.get(view.selected,'id'); local hover_id=G.get(view.hover,'id')
  local commands={}
  for _,p in ipairs(G.get(scene,'packages')) do
    local cb,mb=bands(p); local band=(cb=='red' or mb=='red') and 'red' or (cb=='yellow' or mb=='yellow') and 'yellow' or 'green'
    local pr=G.get(p,'rect')
    -- Pooled package CRAP substitutes mu=20 for unmeasured members, so missing
    -- is counted from the member classes themselves and survives aggregation.
    local count=0
    for _,c in ipairs(G.get(scene,'classes') or {}) do
      if E.equal(G.get(c,'package'),G.get(p,'id')) and missing(c) then count=count+1 end
    end
    local g=grade(p)
    commands[#commands+1]={kind='package',id=G.get(p,'id'),rect=pr,text=G.get(p,'title') or G.get(p,'label'),band=band,red=band=='red',size=F.sizes.title,max_w=G.get(pr,'w')-package_title_reserve,
      missing=count>0,missing_count=count,grade=g,fill=T.fill(g),stroke=T.stroke(g),selected=sel_id~=nil and E.equal(G.get(p,'id'),sel_id)}
    quality(commands,p,'package')
  end
  for _,c in ipairs(G.get(scene,'classes')) do
    local r=G.get(c,'rect'); local cb,mb=bands(c)
    local band=(cb=='red' or mb=='red') and 'red' or (cb=='yellow' or mb=='yellow') and 'yellow' or 'green'
    local id=G.get(c,'id'); local unmeasured=missing(c)
    local g=unmeasured and C.grade_worst or grade(c)
    commands[#commands+1]={kind='class',rect=r,shape=G.name(G.get(c,'shape')),red=band=='red',band=band,id=id,
      missing=unmeasured,grade=g,fill=T.fill(g),stroke=T.stroke(g),
      selected=sel_id~=nil and E.equal(id,sel_id),hovered=hover_id~=nil and E.equal(id,hover_id)}
    quality(commands,c,'class')
    local stereo=G.name(G.get(c,'stereotype')); local marker=stereo=='abstract' or stereo=='interface'
    if marker then commands[#commands+1]={kind='marker',rect=r,text=stereo=='abstract' and 'α' or 'I',x=G.right(r)-44,y=G.get(r,'y')+3,size=12,color=T.pack(T.colors.white)} end
    if G.get(c,'level')~=nil then commands[#commands+1]={kind='level',rect=r,text=tostring(G.get(c,'level')),x=G.get(r,'x')+4,y=G.get(r,'y')+3,size=F.sizes.level,color=T.pack(T.colors.muted)} end
    local slots=M.text_slots(c,r,marker,unmeasured)
    -- The 2px title band runs under the name slot in the box stroke color.
    for _,s in ipairs(slots) do if s.role=='name' and s.top then
      commands[#commands+1]={kind='band',rect=r,x=G.get(r,'x')+6,w=G.get(r,'w')-12,y=math.min(G.bottom(r)-2,s.top+s.h+1),color=T.stroke(g)}; break
    end end
    for _,line in ipairs(slots) do if line.kind=='rule' then line.color=T.stroke(g) end; commands[#commands+1]=line end
    if unmeasured then commands[#commands+1]={kind='no-data',rect=r,text=S.no_data,right=G.right(r)-pad,y=G.bottom(r)-M.no_data_strip-2,size=F.sizes.level,color=T.band_color('red')} end
  end  return commands
end
local role_ink={crap='gold',stereo='muted'}
function M.text_color(role) return T.colors[role_ink[role] or 'ink'] end
-- Natural advance of text at a font size; a font that cannot measure falls back
-- to the monospace estimate (16px Sarasa half-width = layout char_w).
local function measure(font,size,text)
  if font and font.CalcTextSizeA then return font:CalcTextSizeA(size,1e9,0,text,nil,nil).x end
  return L.text_w(text)/16*size
end
-- The layout's 8px/char estimate fixes box widths (JVM goldens); an 18px name
-- may be wider, so the drawn size shrinks until the text, bold offset and
-- italic shear fit inside max_w. Never grows past the nominal size.
function M.fit(font,size,text,max_w,bold,italic)
  local extra=(bold and size*.05 or 0)+(italic and size*.2 or 0)
  local natural=measure(font,size,text)+extra
  if max_w and natural>max_w and natural>0 then return size*math.max(0,max_w)/natural end
  return size
end
function M.new(imgui,fonts)
  fonts=fonts or {}
  local function vec(x,y) return imgui.ImVec2_Float(x,y) end
  local api={}
  api.draw=function(scene,width,height,view)
    -- view.ui is the DPI factor (domain.display): world->window px multiplies
    -- zoom by it once; strokes are screen px times ui and never zoom.
    local ui=view and view.ui or 1
    local size=G.get(scene,'size'); local scale=view and (view.zoom or 1)*ui or math.min(1,width/G.get(size,'w'),height/G.get(size,'h'))
    local list=imgui.GetBackgroundDrawList_Nil()
    local W=T.dpi(ui)
    local function point(x,y) return vec((x-(view and view.cam_x or 0))*scale,(y-(view and view.cam_y or 0))*scale) end
    local function text(font,size,max_w,x,y,color,s,italic,bold,center)
      size=M.fit(font,size,s,max_w,bold,italic)
      local tw=center and measure(font,size,s) or 0
      for pass=1,bold and 2 or 1 do
        local origin=point(x-tw/2+(pass-1)*size*.05,y)
        local first=italic and list.VtxBuffer and list.VtxBuffer.Size
        list:AddText_FontPtr(font,size*scale,origin,color,s,nil,0,nil)
        -- Shear only this text's newly emitted glyphs: portable oblique styling
        -- without loading or redistributing a platform-specific italic font.
        if first then for i=first,list.VtxBuffer.Size-1 do local pos=list.VtxBuffer.Data[i].pos; pos.x=pos.x+(origin.y+size*scale-pos.y)*.2 end end
      end
    end
    local function dashed_rect(r,color,width)
      local x,y,w,h=G.get(r,'x'),G.get(r,'y'),G.get(r,'w'),G.get(r,'h')
      -- Dash pattern in screen px, like the widths: converted to world units.
      local dash,gap=W.dash/scale,W.dash_gap/scale
      local function dash_line(x1,y1,x2,y2)
        local dx,dy=x2-x1,y2-y1; local len=math.sqrt(dx*dx+dy*dy); if len==0 then return end
        local ux,uy=dx/len,dy/len; local t=0
        while t<len do local e=math.min(t+dash,len); local a=point(x1+ux*t,y1+uy*t); local b=point(x1+ux*e,y1+uy*e); list:AddLine(a,b,color,width); t=e+gap end
      end
      -- imgui has no dash primitive: emit AddLine segments along each side.
      dash_line(x,y,x+w,y); dash_line(x+w,y,x+w,y+h)
      dash_line(x+w,y+h,x,y+h); dash_line(x,y+h,x,y)
    end
    for _,c in ipairs(M.commands(scene,view)) do
      if c.kind=='package' or c.kind=='class' then
        local r=c.rect; local x,y,w,h=G.get(r,'x'),G.get(r,'y'),G.get(r,'w'),G.get(r,'h')
        local package=c.kind=='package'
        local radius=(package and T.radius.package or T.radius.class)*scale
        local stroke=T.pack(c.stroke,package and T.alpha.package_stroke or 255)
        local line_w=package and W.package or W.class
        if c.selected then stroke=T.pack(T.colors.gold); line_w=W.selected
        elseif c.hovered then stroke=T.pack(T.colors.ink) end
        if c.shape=='oval' then
          list:AddEllipseFilled(point(x+w/2,y+h/2),w/2*scale,h/2*scale,T.pack(T.fill(nil)),0,0)
          list:AddEllipse(point(x+w/2,y+h/2),w/2*scale,h/2*scale,c.selected and stroke or c.hovered and stroke or T.pack(T.stroke(nil)),0,0,line_w)
        else
          -- Continuous quality fill (namespace frames are washed, classes opaque).
          list:AddRectFilled(point(x,y),point(x+w,y+h),T.pack(c.fill,package and T.alpha.package_fill or 255),radius,0)
          if c.missing then dashed_rect(r,stroke,line_w)
          else list:AddRect(point(x,y),point(x+w,y+h),stroke,radius,0,line_w) end
        end
        if package then text(fonts.title,c.size,c.max_w,x+pad,y+8,T.pack(T.colors.gold),c.text or '',true) end
      elseif c.kind=='quality' then
        list:AddCircleFilled(point(c.x,c.y),c.radius*scale,T.pack(T.fill(c.grade)),0)
        if list.AddCircle then list:AddCircle(point(c.x,c.y),c.radius*scale,T.pack(T.stroke(c.grade)),0,W.dot) end
        local font=fonts.badge or fonts.body
        text(font,F.sizes.badge,nil,c.x-measure(font,F.sizes.badge,c.text)/2,c.y-F.sizes.badge/2,T.pack(T.colors.white),c.text)
      elseif c.kind=='band' then list:AddLine(point(c.x,c.y),point(c.x+c.w,c.y),T.pack(c.color),W.band)
      elseif c.kind=='rule' then list:AddLine(point(c.x+6,c.y),point(c.x+c.w-6,c.y),T.pack(c.color or T.stroke(nil)),W.rule)
      elseif c.kind=='text' then
        -- Fit the slot height and width budget, then centre every line inside
        -- its slot (original centred text); slots already exclude the badges.
        local font=c.role=='name' and fonts.title or fonts.body
        local size=M.fit(font,math.min(c.size,c.h or c.size),c.text,c.max_w,c.bold,c.italic)
        local cx=c.max_w and (c.x+c.max_w/2) or nil
        text(font,size,c.max_w,cx or c.x,(c.top or c.y)+((c.h or size)-size)/2,T.pack(M.text_color(c.role)),c.text,c.italic,c.bold,cx~=nil)
      elseif c.kind=='no-data' then
        local font=fonts.level or fonts.body
        text(font,c.size,nil,c.right-measure(font,c.size,c.text),c.y,c.color,c.text)
      elseif c.kind=='marker' or c.kind=='level' then
        text((c.kind=='marker' and fonts.marker or fonts.level) or fonts.body,c.size or 13,nil,c.x,c.y,c.color or 0xffeeeeee,c.text)
      end
    end
  end
  -- The back label, drawn on the same draw list as the canvas.
  -- Its 12px inset and body size are view units, scaled by ui to window px.
  api.back=function(ui)
    ui=ui or 1
    imgui.GetBackgroundDrawList_Nil():AddText_FontPtr(fonts.body,F.sizes.body*ui,vec(12*ui,12*ui),0xffffffff,S.back,nil,0,nil)
  end
  return api
end
return M
