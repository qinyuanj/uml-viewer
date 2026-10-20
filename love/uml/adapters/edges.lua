-- cimgui draw-list boundary; scene geometry remains independent of the binding.
-- Colors and screen-px widths come from the theme; strokes never scale with
-- zoom, only world-unit geometry does.
local G=require('uml.domain.geom')
local T=require('uml.domain.theme')
local M={}
local g=G.get
local Highlight=require('uml.application.highlight')
-- Violation semantics win: a violating edge keeps its red family when hot, and
-- a hover lift never thins it below its violation width.
local function edge_style(violating,hot,warm)
  local C,W=T.colors,T.widths
  if violating then return T.pack(hot and C.edge_violating_hot or C.edge_violating),hot and W.edge_violating_hot or W.edge_violating end
  if hot then return T.pack(C.edge_hot),W.edge_hot end
  if warm then return T.pack(C.edge_warm),W.edge_warm end
  return T.pack(C.edge),W.edge
end
-- `view` is {selected=hit, hover=hit}; a function is the legacy hot predicate.
function M.commands(scene,view)
  local out={}
  if g(g(scene,'diagram'),'hide-edges') then
    for _,ind in ipairs(g(scene,'dep-indicators') or {}) do out[#out+1]={kind='indicator',points=g(ind,'triangle'),violating=g(ind,'violating')} end
    return out
  end
  local h=type(view)=='table' and Highlight.new(scene,view) or nil
  for _,e in ipairs(g(scene,'edges') or {}) do
    local hot,warm,dimmed
    if type(view)=='function' then hot=view(e)
    elseif h then hot=h.hot(e); warm=not hot and h.warm(e); dimmed=h.focus~=nil and not hot end
    local violating=g(e,'violating')
    local color,width=edge_style(violating,hot,warm)
    local from,to=g(e,'from'),g(e,'to')
    for _,ps in ipairs(g(e,'strokes') or {}) do out[#out+1]={kind='stroke',from=from,to=to,points=ps,color=color,width=width,hot=hot or nil,warm=warm or nil,dimmed=dimmed or nil} end
    if g(e,'head') and g(e,'tip') then out[#out+1]={kind='head',from=from,to=to,head=G.name(g(e,'head')),tip=g(e,'tip'),behind=g(e,'behind'),color=color,width=width,hot=hot or nil,warm=warm or nil,dimmed=dimmed or nil} end
  end; return out
end
function M.draw(imgui,list,scene,ox,oy,scale,selected)
  ox,oy,scale=ox or 0,oy or 0,scale or 1
  local function point(p) return imgui.ImVec2_Float(ox+p[1]*scale,oy+p[2]*scale) end
  -- Widths are fixed screen px from the theme; geometry alone scales.
  local function line(a,b,color,width) list:AddLine(point(a),point(b),color,width) end
  local function triangle(a,b,c,fill,color,width)
    list:AddTriangleFilled(point(a),point(b),point(c),fill)
    list:AddTriangle(point(a),point(b),point(c),color,width)
  end
  for _,cmd in ipairs(M.commands(scene,selected)) do
    if cmd.dimmed then cmd.color=T.fade(cmd.color) end
    if cmd.kind=='stroke' then for i=1,#cmd.points-1 do line(cmd.points[i],cmd.points[i+1],cmd.color,cmd.width) end
    elseif cmd.kind=='indicator' then local p=cmd.points; local color=cmd.violating and T.pack(T.colors.band.red) or T.pack(T.colors.indicator); triangle(p[1],p[2],p[3],color,color,1)
    else local p,b=cmd.tip,cmd.behind; local dx,dy=p[1]-b[1],p[2]-b[2]; local len=math.sqrt(dx*dx+dy*dy); if len==0 then dx,dy,len=1,0,1 end; dx,dy=dx/len,dy/len
      local cos,sin=math.cos(math.pi/9),math.sin(math.pi/9)
      local a={p[1]-16*(dx*cos-dy*sin),p[2]-16*(dy*cos+dx*sin)}; local c={p[1]-16*(dx*cos+dy*sin),p[2]-16*(dy*cos-dx*sin)}
      local bg=T.pack(T.colors.canvas_bg); if cmd.dimmed then bg=T.fade(bg) end
      if cmd.head=='triangle' then triangle(p,a,c,bg,cmd.color,cmd.width)
      elseif cmd.head=='diamond' or cmd.head=='diamond-fill' then local back={p[1]-16*dx,p[2]-16*dy}; local fill=cmd.head=='diamond-fill' and (cmd.dimmed and T.fade(T.pack(T.colors.ink)) or T.pack(T.colors.ink)) or bg; list:AddQuadFilled(point(p),point(a),point(back),point(c),fill); list:AddQuad(point(p),point(a),point(back),point(c),cmd.color,cmd.width)
      else line(p,a,cmd.color,cmd.width); line(p,c,cmd.color,cmd.width) end
    end
  end
end
return M
