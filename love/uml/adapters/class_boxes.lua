local E=require('uml.edn')
local G=require('uml.domain.geom')
local M={}
local C=require('uml.domain.config')
local L=require('uml.engine.layout')
local S=require('uml.application.strings')
local F=require('uml.adapters.fonts')
-- Text keeps the layout's 12px side padding; package titles also clear the C/M dots.
local pad,package_title_reserve=12,48
local function bands(c)
  return G.name(C.crap_band(C.crap_risk(G.get(c,'crap')))),G.name(C.mutation_band(C.mutation_ratio(c)))
end
local function quality(commands,c,owner)
  local r=G.get(c,'rect'); local cb,mb=bands(c)
  for i,b in ipairs{cb,mb} do commands[#commands+1]={kind='quality',owner=owner,id=G.get(c,'id'),rect=r,text=i==1 and 'C' or 'M',band=b,radius=6,x=G.right(r)-24+(i-1)*14,y=G.get(r,'y')+10} end
end
-- The corner band holds the level (top-left) and the C/M dots and α/I marker
-- (top-right); text whose slot reaches into it must stop short of the badges.
local band_bottom,dots_reserve,marker_reserve=16,32,46
-- Vertical slots for a class's text lines. The layout keeps its golden 18px
-- line pitch, so the 18px names and members are packed into each
-- run of lines between rules (and the corner band / box bottom): nominal sizes
-- when they fit, scaled down together when they do not, with even gaps.
function M.text_slots(c,r,marker)
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
      local e={kind='text',role=kind,italic=italic,bold=kind=='name',size=kind=='name' and F.sizes.title or F.sizes.body,x=x+pad,y=y,text=G.get(line,'text') or ''}
      out[#out+1]=e; run[#run+1]=e; y=y+18
    end
  end
  flush(ry+h-4)
  return out
end-- Adapter contract: a renderer consumes commands; no UI binding leaks into scenes.
function M.commands(scene)
  local commands={}
  for _,p in ipairs(G.get(scene,'packages')) do
    local cb,mb=bands(p); local band=(cb=='red' or mb=='red') and 'red' or (cb=='yellow' or mb=='yellow') and 'yellow' or 'green'
    local pr=G.get(p,'rect')
    commands[#commands+1]={kind='package',rect=pr,text=G.get(p,'title') or G.get(p,'label'),band=band,red=band=='red',size=F.sizes.title,max_w=G.get(pr,'w')-package_title_reserve}
    quality(commands,p,'package')
  end
  for _,c in ipairs(G.get(scene,'classes')) do
    local r=G.get(c,'rect'); local cb=G.name(C.crap_band(C.crap_risk(G.get(c,'crap')))); local mb=G.name(C.mutation_band(C.mutation_ratio(c)))
    local band=(cb=='red' or mb=='red') and 'red' or (cb=='yellow' or mb=='yellow') and 'yellow' or 'green'
    commands[#commands+1]={kind='class',rect=r,shape=G.name(G.get(c,'shape')),red=band=='red',band=band,id=G.get(c,'id')}
    quality(commands,c,'class')
    local stereo=G.name(G.get(c,'stereotype')); local marker=stereo=='abstract' or stereo=='interface'
    if marker then commands[#commands+1]={kind='marker',rect=r,text=stereo=='abstract' and 'α' or 'I',x=G.right(r)-44,y=G.get(r,'y')+3,size=12,color=0xffffffff} end
    if G.get(c,'level')~=nil then commands[#commands+1]={kind='level',rect=r,text=tostring(G.get(c,'level')),x=G.get(r,'x')+4,y=G.get(r,'y')+3,size=F.sizes.level,color=0xffeeeeee} end
    for _,line in ipairs(M.text_slots(c,r,marker)) do commands[#commands+1]=line end
  end  return commands
end
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
    local size=G.get(scene,'size'); local scale=view and (view.zoom or 1) or math.min(1,width/G.get(size,'w'),height/G.get(size,'h'))
    local list=imgui.GetBackgroundDrawList_Nil()
    local function point(x,y) return vec((x-(view and view.cam_x or 0))*scale,(y-(view and view.cam_y or 0))*scale) end
    local function text(font,size,max_w,x,y,color,s,italic,bold)
      size=M.fit(font,size,s,max_w,bold,italic)
      for pass=1,bold and 2 or 1 do
        local origin=point(x+(pass-1)*size*.05,y)
        local first=italic and list.VtxBuffer.Size
        list:AddText_FontPtr(font,size*scale,origin,color,s,nil,0,nil)
        -- Shear only this text's newly emitted glyphs: portable oblique styling
        -- without loading or redistributing a platform-specific italic font.
        if first then for i=first,list.VtxBuffer.Size-1 do local pos=list.VtxBuffer.Data[i].pos; pos.x=pos.x+(origin.y+size*scale-pos.y)*.2 end end
      end
    end
    for _,c in ipairs(M.commands(scene)) do
      if c.kind=='package' or c.kind=='class' then
        local r=c.rect; local x,y,w,h=G.get(r,'x'),G.get(r,'y'),G.get(r,'w'),G.get(r,'h')
        local color=c.red and 0xff5555dd or c.band=='yellow' and 0xff55ccee or 0xff77bb77
        if c.shape=='oval' then list:AddEllipse(point(x+w/2,y+h/2),w/2*scale,h/2*scale,color,0,0,1.5)
        else list:AddRect(point(x,y),point(x+w,y+h),color,0,0,1.5) end
        if c.kind=='package' then text(fonts.title,c.size,c.max_w,x+pad,y+8,0xffdddddd,c.text or '') end
      elseif c.kind=='quality' then
        local color=c.band=='red' and 0xff5555dd or c.band=='yellow' and 0xff55ccee or 0xff77bb77
        list:AddCircleFilled(point(c.x,c.y),c.radius*scale,color,0)
        text(fonts.badge or fonts.body,F.sizes.badge,nil,c.x-3,c.y-4.5,0xffffffff,c.text)
      elseif c.kind=='rule' then list:AddLine(point(c.x,c.y),point(c.x+c.w,c.y),0xff777777,1)
      elseif c.kind=='text' then
        -- Fit the slot height and the width budget, then centre in the slot.
        local font=c.role=='name' and fonts.title or fonts.body
        local size=M.fit(font,math.min(c.size,c.h or c.size),c.text,c.max_w,c.bold,c.italic)
        text(font,size,c.max_w,c.x,(c.top or c.y)+((c.h or size)-size)/2,c.color or 0xffeeeeee,c.text,c.italic,c.bold)
      elseif c.kind=='marker' or c.kind=='level' then
        text((c.kind=='marker' and fonts.marker or fonts.level) or fonts.body,c.size or 13,nil,c.x,c.y,c.color or 0xffeeeeee,c.text)
      end
    end
  end
  -- The back label, drawn on the same draw list as the canvas.
  api.back=function()
    imgui.GetBackgroundDrawList_Nil():AddText_FontPtr(fonts.body,F.sizes.body,vec(12,12),0xffffffff,S.back,nil,0,nil)
  end
  return api
end
return M
