-- cimgui draw-list boundary; scene geometry remains independent of the binding.
-- Colors and screen-px widths come from the theme; strokes never scale with
-- zoom, only world-unit geometry does.
local G=require('uml.domain.geom')
local T=require('uml.domain.theme')
local M={}
local g=G.get
local function edge_style(violating,hot)
  local C=T.colors
  local color=violating and (hot and C.edge_violating_hot or C.edge_violating) or (hot and C.edge_hot or C.edge)
  local width=violating and (hot and T.widths.edge_violating_hot or T.widths.edge_violating) or (hot and T.widths.edge_hot or T.widths.edge)
  return T.pack(color),width
end
function M.commands(scene,selected)
  local out={}
  if g(g(scene,'diagram'),'hide-edges') then
    for _,ind in ipairs(g(scene,'dep-indicators') or {}) do out[#out+1]={kind='indicator',points=g(ind,'triangle'),violating=g(ind,'violating')} end
    return out
  end
  for _,e in ipairs(g(scene,'edges') or {}) do
    local hot=selected and selected(e); local violating=g(e,'violating')
    local color,width=edge_style(violating,hot)
    for _,ps in ipairs(g(e,'strokes') or {}) do out[#out+1]={kind='stroke',points=ps,color=color,width=width} end
    if g(e,'head') and g(e,'tip') then out[#out+1]={kind='head',head=G.name(g(e,'head')),tip=g(e,'tip'),behind=g(e,'behind'),color=color,width=width} end
  end; return out
end
-- scale is world->window px (zoom times ui); ui is the DPI factor that turns
-- the theme's screen-px widths into window px.
function M.draw(imgui,list,scene,ox,oy,scale,selected,ui)
  ox,oy,scale,ui=ox or 0,oy or 0,scale or 1,ui or 1
  local function point(p) return imgui.ImVec2_Float(ox+p[1]*scale,oy+p[2]*scale) end
  -- Widths are fixed screen px from the theme; geometry alone scales.
  local function line(a,b,color,width) list:AddLine(point(a),point(b),color,width*ui) end
  local function triangle(a,b,c,fill,color,width)
    list:AddTriangleFilled(point(a),point(b),point(c),fill)
    list:AddTriangle(point(a),point(b),point(c),color,width*ui)
  end
  for _,cmd in ipairs(M.commands(scene,selected)) do
    if cmd.kind=='stroke' then for i=1,#cmd.points-1 do line(cmd.points[i],cmd.points[i+1],cmd.color,cmd.width) end
    elseif cmd.kind=='indicator' then local p=cmd.points; local color=cmd.violating and T.pack(T.colors.band.red) or T.pack(T.colors.indicator); triangle(p[1],p[2],p[3],color,color,1)
    else local p,b=cmd.tip,cmd.behind; local dx,dy=p[1]-b[1],p[2]-b[2]; local len=math.sqrt(dx*dx+dy*dy); if len==0 then dx,dy,len=1,0,1 end; dx,dy=dx/len,dy/len
      local cos,sin=math.cos(math.pi/9),math.sin(math.pi/9)
      local a={p[1]-16*(dx*cos-dy*sin),p[2]-16*(dy*cos+dx*sin)}; local c={p[1]-16*(dx*cos+dy*sin),p[2]-16*(dy*cos-dx*sin)}
      if cmd.head=='triangle' then triangle(p,a,c,T.pack(T.colors.canvas_bg),cmd.color,T.widths.edge)
      elseif cmd.head=='diamond' or cmd.head=='diamond-fill' then local back={p[1]-16*dx,p[2]-16*dy}; local fill=cmd.head=='diamond-fill' and 0xffe4ecec or T.pack(T.colors.canvas_bg); list:AddQuadFilled(point(p),point(a),point(back),point(c),fill); list:AddQuad(point(p),point(a),point(back),point(c),cmd.color,cmd.width*ui)
      else line(p,a,cmd.color,cmd.width); line(p,c,cmd.color,cmd.width) end
    end
  end
end
return M
