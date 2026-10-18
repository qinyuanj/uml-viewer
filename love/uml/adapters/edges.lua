-- cimgui draw-list boundary; scene geometry remains independent of the binding.
local G=require('uml.domain.geom')
local M={}
local g=G.get
function M.commands(scene,selected)
  local out={}
  if g(g(scene,'diagram'),'hide-edges') then
    for _,ind in ipairs(g(scene,'dep-indicators') or {}) do out[#out+1]={kind='indicator',points=g(ind,'triangle'),violating=g(ind,'violating')} end
    return out
  end
  for _,e in ipairs(g(scene,'edges') or {}) do
    local hot=selected and selected(e); local violating=g(e,'violating'); local color=violating and (hot and 0xff3040ff or 0xff242ac4) or (hot and 0xff48c4e8 or 0xffa8b89d); local width=violating and (hot and 3.2 or 2) or (hot and 2.2 or 1.4)
    for _,ps in ipairs(g(e,'strokes') or {}) do out[#out+1]={kind='stroke',points=ps,color=color,width=width} end
    if g(e,'head') and g(e,'tip') then out[#out+1]={kind='head',head=G.name(g(e,'head')),tip=g(e,'tip'),behind=g(e,'behind'),color=color,width=width} end
  end; return out
end
function M.draw(imgui,list,scene,ox,oy,scale,selected)
  ox,oy,scale=ox or 0,oy or 0,scale or 1
  local function point(p) return imgui.ImVec2_Float(ox+p[1]*scale,oy+p[2]*scale) end
  local function line(a,b,color,width) list:AddLine(point(a),point(b),color,width*scale) end
  local function triangle(a,b,c,fill,color,width)
    list:AddTriangleFilled(point(a),point(b),point(c),fill)
    list:AddTriangle(point(a),point(b),point(c),color,width*scale)
  end
  for _,cmd in ipairs(M.commands(scene,selected)) do
    if cmd.kind=='stroke' then for i=1,#cmd.points-1 do line(cmd.points[i],cmd.points[i+1],cmd.color,cmd.width) end
    elseif cmd.kind=='indicator' then local p=cmd.points; local color=cmd.violating and 0xff5555dd or 0xffaaa49c; triangle(p[1],p[2],p[3],color,color,1)
    else local p,b=cmd.tip,cmd.behind; local dx,dy=p[1]-b[1],p[2]-b[2]; local len=math.sqrt(dx*dx+dy*dy); if len==0 then dx,dy,len=1,0,1 end; dx,dy=dx/len,dy/len
      local cos,sin=math.cos(math.pi/9),math.sin(math.pi/9)
      local a={p[1]-16*(dx*cos-dy*sin),p[2]-16*(dy*cos+dx*sin)}; local c={p[1]-16*(dx*cos+dy*sin),p[2]-16*(dy*cos-dx*sin)}
      if cmd.head=='triangle' then triangle(p,a,c,0xff201c16,cmd.color,1.5)
      elseif cmd.head=='diamond' or cmd.head=='diamond-fill' then local back={p[1]-16*dx,p[2]-16*dy}; local fill=cmd.head=='diamond-fill' and 0xffe4ecec or 0xff201c16; list:AddQuadFilled(point(p),point(a),point(back),point(c),fill); list:AddQuad(point(p),point(a),point(back),point(c),cmd.color,cmd.width*scale)
      else line(p,a,cmd.color,cmd.width); line(p,c,cmd.color,cmd.width) end
    end
  end
end
return M
