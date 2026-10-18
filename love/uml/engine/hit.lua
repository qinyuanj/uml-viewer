local E=require('uml.edn')
local G=require('uml.domain.geom')
local M={}
local g,m,v,k=G.get,G.map,E.vector,E.keyword
function M.deps_of(e)
  local deps=g(e,'deps'); if deps and #deps>0 then return deps end
  if g(e,'from') and g(e,'to') then return v{m{from=g(e,'from'),to=g(e,'to'),violating=not not g(e,'violating')}} end
end
function M.class_by_id(scene,id) for _,c in ipairs(g(scene,'classes') or {}) do if E.equal(id,g(c,'id')) then return c end end end
function M.package_by_id(scene,id) for _,c in ipairs(g(scene,'packages') or {}) do if E.equal(id,g(c,'id')) then return c end end end
function M.connected_edges(scene,id) local out=v(); for _,e in ipairs(g(scene,'edges') or {}) do if E.equal(id,g(e,'from')) or E.equal(id,g(e,'to')) then out[#out+1]=e end end; return out end
function M.dep_indicators(scene)
  local out=v(); local es=g(scene,'edges') or {}
  for _,c in ipairs(g(scene,'classes') or {}) do local r=g(c,'rect'); if r then
    for _,dir in ipairs{'in','out'} do local endpoint=dir=='in' and 'to' or 'from'; local box,leaf=v(),v(); local id=g(c,'id')
      for _,e in ipairs(es) do for _,dep in ipairs(M.deps_of(e) or {}) do if E.equal(id,g(e,endpoint)) then box[#box+1]=dep end; if E.equal(id,g(dep,endpoint)) then leaf[#leaf+1]=dep end end end
      local deps=#leaf>0 and leaf or box
      if #deps>0 then local x,y,b=G.cx(r),g(r,'y'),G.bottom(r); local h=12*.8660254037844386; local tri=dir=='in' and v{v{x,y-h},v{x-6,y},v{x+6,y}} or v{v{x-6,b},v{x+6,b},v{x,b+h}}; local violating=false; for _,dep in ipairs(deps) do if g(dep,'violating') then violating=true end end
        out[#out+1]=m{id=id,dir=k(dir),triangle=tri,deps=deps,violating=violating}
      end
    end
  end end; return out
end
function M.indicator_at(scene,x,y,pad)
  local inds=g(scene,'dep-indicators') or {}; for i=#inds,1,-1 do local a=inds[i]; if G.near_triangle({x,y},g(a,'triangle'),pad or 5) then return m{kind=k('dep'),dir=g(a,'dir'),id=g(a,'id'),deps=g(a,'deps'),violating=g(a,'violating')} end end
end
function M.edge_at(scene,x,y,pad)
  local es=g(scene,'edges') or {}; for i=#es,1,-1 do local e=es[i]; local paths=g(e,'strokes'); if not paths or #paths==0 then local ps=g(e,'points'); paths=ps and #ps>1 and {ps} or {} end
    for _,ps in ipairs(paths) do if G.near_polyline({x,y},ps,pad or 8) then return m{kind=k('edge'),from=g(e,'from'),to=g(e,'to'),deps=M.deps_of(e) or v()} end end
  end
end
function M.at(scene,x,y)
  local ind=M.indicator_at(scene,x,y); if ind then return ind end
  local cs=g(scene,'classes') or {}; local visible=false; for _,c in ipairs(cs) do if not g(c,'dummy?') then visible=true end end
  for i=#cs,1,-1 do local c=cs[i]; if not visible or not g(c,'dummy?') then
    for _,dir in ipairs{'in','out'} do for _,p in ipairs(g(c,dir..'-ports') or {}) do if G.contains(g(p,'rect'),x,y) then return m{kind=k('port'),id=g(p,'id'),parent=g(c,'id'),dir=k(dir)} end end end
    local r=g(c,'rect'); if G.contains(r,x,y) then
      local ly=g(r,'y')+16; for _,line in ipairs(g(c,'lines') or {}) do if G.name(g(line,'kind'))=='rule' then ly=ly+12 else if y>=ly and y<=ly+17 and G.name(g(line,'kind'))=='child' then return m{kind=k('child'),id=g(line,'id'),parent=g(c,'id'),['drill?']=not not g(line,'drill?')} end; ly=ly+18 end end
      return m{kind=k('class'),id=g(c,'id'),['drill?']=not not g(c,'drill?')}
    end
  end end
  local edge=M.edge_at(scene,x,y); if edge then return edge end
  local ps=g(scene,'packages') or {}; for i=#ps,1,-1 do if G.contains(g(ps[i],'rect'),x,y) then return m{kind=k('package'),id=g(ps[i],'id')} end end
end
return M
