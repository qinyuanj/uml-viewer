-- Viewer document compiler: tagged IR and view options in, tagged scene out.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local H=require('uml.domain.hierarchy')
local L=require('uml.engine.layout')
local R=require('uml.engine.route')
local Curve=require('uml.engine.curve')
local M={}
local g,m,put=G.get,G.map,G.put
local V,K=E.vector,E.keyword
local function translate(scene,dx,dy)
  for _,k in ipairs{'packages','classes'} do for _,x in ipairs(g(scene,k)) do put(x,'rect',G.move(g(x,'rect'),dx,dy)) end end
  for _,e in ipairs(g(scene,'edges') or {}) do local ps=g(e,'points'); if ps then for i,p in ipairs(ps) do ps[i]=V{p[1]+dx,p[2]+dy} end end end
  return scene
end
local function bounds(scene)
  local rs={}; local idx={}; for _,k in ipairs{'packages','classes'} do for _,x in ipairs(g(scene,k)) do rs[#rs+1]=g(x,'rect'); if k=='classes' then idx[E.encode(g(x,'id'))]=g(x,'rect') end end end
  for _,e in ipairs(g(scene,'edges') or {}) do local b=Curve.path_bounds(g(e,'points'),idx[E.encode(g(e,'from'))],idx[E.encode(g(e,'to'))]); if b then rs[#rs+1]=b end end
  return G.inflate(G.union(rs) or G.rect(16,16,368,268),16)
end
local function size(scene)
  local b=bounds(scene); put(scene,'size',m{w=G.right(b)+40,h=G.bottom(b)+40,['min-x']=math.min(0,g(b,'x')-40),['min-y']=math.min(0,g(b,'y')-40)})
  return scene
end
local function compile(diagram)
  local scene=R.route(L.layout(diagram)); local b=bounds(scene)
  translate(scene,math.max(0,40-g(b,'x')),math.max(0,40-g(b,'y')))
  return size(scene)
end
function M.compile_view(doc,metrics_root,focus,extra)
  focus=focus or V(); extra=extra or E.map()
  if g(doc,'hierarchical') then
    local view
    if extra==true or g(extra,'proposal') then
      view=g(extra,'open-layer') and H.layer_view(doc,g(extra,'proposal-id'),g(extra,'open-layer')) or H.proposal_view(doc,g(extra,'proposal-id'))
    else view=H.view_at(doc,focus) end
    return compile(H.apply_declutter(view,g(extra,'declutter')))
  end
  local sections,classes,packages=V(),V(),V(); local y=24
  for i,d in ipairs(g(doc,'diagrams') or V{doc}) do
    local scene=compile(d); local h=g(g(scene,'size'),'h'); translate(scene,40,y+40)
    put(scene,'title',g(d,'title')); put(scene,'title-y',y); sections[#sections+1]=scene
    for _,k in ipairs{'packages','classes'} do for _,x in ipairs(g(scene,k)) do
      put(x,'id',K('d'..(i-1)..'-'..G.name(g(x,'id')))); if g(x,'package') then put(x,'package',K('d'..(i-1)..'-'..G.name(g(x,'package')))) end
      local dst=k=='packages' and packages or classes; dst[#dst+1]=x
    end end
    y=y+40+h+64
  end
  local scene=size(m{diagram=m{title=g(doc,'title')},sections=sections,classes=classes,packages=packages,edges=V()}); put(g(scene,'size'),'h',y); return scene
end
-- A rejected reload leaves the last usable scene and timestamp intact for retry.
function M.new(doc,mtime,focus,extra)
  return {document=doc,mtime=mtime,focus=focus or V(),extra=extra or E.map(),scene=M.compile_view(doc,nil,focus,extra),revision=1}
end
function M.reload(state,text,mtime)
  if mtime==state.mtime then return state,false end
  local ok,next_state=pcall(function() local doc=require('uml.domain.ir').read_document(text); local s=M.new(doc,mtime,state.focus,state.extra); s.revision=state.revision+1; return s end)
  if ok then return next_state,true end
  return state,false,tostring(next_state)
end
return M
