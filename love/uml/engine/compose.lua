-- Scene compilation shared by document loading, rendering and selection.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local L=require('uml.engine.layout')
local R=require('uml.engine.route')
local C=require('uml.engine.curve')
local M={}
local g,m,put,v=G.get,G.map,G.put,E.vector
local function index(scene,key) local out={}; for _,c in ipairs(g(scene,key) or {}) do out[E.encode(g(c,'id'))]=c end; return out end
local function path(scene,e)
  local ps=g(e,'points'); if not ps or #ps<2 then return end
  local idx=index(scene,'classes'); local a,b=idx[E.encode(g(e,'from'))],idx[E.encode(g(e,'to'))]
  return C.constrain_ends(C.basis_path(ps),a and g(a,'rect'),b and g(b,'rect'))
end
local function translate(scene,dx,dy)
  for _,key in ipairs{'classes','packages'} do for _,x in ipairs(g(scene,key)) do put(x,'rect',G.move(g(x,'rect'),dx,dy)) end end
  for _,e in ipairs(g(scene,'edges')) do local ps=v(); for _,p in ipairs(g(e,'points')) do ps[#ps+1]=v{p[1]+dx,p[2]+dy} end; put(e,'points',ps) end
  return scene
end
local function bounds(scene)
  local rs={}; for _,key in ipairs{'classes','packages'} do for _,c in ipairs(g(scene,key)) do rs[#rs+1]=g(c,'rect') end end
  for _,e in ipairs(g(scene,'edges')) do local p=path(scene,e); if p then rs[#rs+1]=C.path_bounds(p) end end
  local b=G.union(rs); return b and G.inflate(b,16) or G.rect(0,0,400,300)
end
local function size(scene)
  local b=bounds(scene); put(scene,'size',m{w=G.right(b)+40,h=G.bottom(b)+40,['min-x']=math.min(0,g(b,'x')-40),['min-y']=math.min(0,g(b,'y')-40)}); return scene
end
local function fit(scene) local b=bounds(scene); translate(scene,math.max(0,40-g(b,'x')),math.max(0,40-g(b,'y'))); return size(scene) end
function M.prepare_scene(scene)
  local cs=g(scene,'classes'); local es=v()
  for _,raw in ipairs(g(scene,'edges')) do local e=G.copy(raw); local p=path(scene,e)
    if p then local bb=C.path_bounds(p); local probe=G.inflate(bb,13); local rects={}
      for _,c in ipairs(cs) do if not g(c,'dummy?') and not E.equal(g(c,'id'),g(e,'from')) and not E.equal(g(c,'id'),g(e,'to')) and G.overlaps(probe,g(c,'rect')) then rects[#rects+1]=g(c,'rect') end end
      local tangent=C.end_tangent(p); put(e,'behind',tangent[1]); put(e,'tip',tangent[2]); put(e,'draw-bounds',bb); put(e,'strokes',G.gap_polyline(C.flatten_path(p),rects,5))
    end; es[#es+1]=e
  end
  put(scene,'edges',es)
  for _,pair in ipairs{{'classes','class-by-id'},{'packages','package-by-id'}} do local idx=E.map(); for _,c in ipairs(g(scene,pair[1])) do E.assoc(idx,g(c,'id'),c) end; put(scene,pair[2],idx) end
  return scene
end
function M.compile_diagram(diagram)
  local scene=M.prepare_scene(fit(R.route(L.layout(diagram))))
  if g(diagram,'hide-edges') then put(scene,'dep-indicators',require('uml.engine.hit').dep_indicators(scene)) end; return scene
end
function M.compile_document(doc)
  local sections,classes,packages,edges=v(),v(),v(),v(); local y=24
  for i,d in ipairs(g(doc,'diagrams') or v{doc}) do
    local scene=fit(R.route(L.layout(d))); local h=g(g(scene,'size'),'h')
    local function q(id) return E.keyword('d'..(i-1)..'-'..G.name(id)) end
    for _,key in ipairs{'packages','classes'} do for _,c in ipairs(g(scene,key)) do put(c,'id',q(g(c,'id'))); if g(c,'package') then put(c,'package',q(g(c,'package'))) end end end
    for _,e in ipairs(g(scene,'edges')) do put(e,'from',q(g(e,'from'))); put(e,'to',q(g(e,'to'))) end
    translate(scene,40,y+40); put(scene,'title',g(d,'title')); put(scene,'crap',g(d,'crap')); put(scene,'title-y',y); sections[#sections+1]=scene
    for _,pair in ipairs{{'classes',classes},{'packages',packages},{'edges',edges}} do for _,x in ipairs(g(scene,pair[1])) do pair[2][#pair[2]+1]=x end end; y=y+40+h+64
  end
  local scene=m{title=g(doc,'title'),diagram=m{title=g(doc,'title')},sections=sections,classes=classes,packages=packages,edges=edges}; local b=bounds(scene); local dx=math.max(0,40-g(b,'x')); if dx>0 then translate(scene,dx,0) end
  size(scene); put(g(scene,'size'),'h',y); return M.prepare_scene(scene)
end
return M
