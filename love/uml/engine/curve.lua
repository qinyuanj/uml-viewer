-- Portable basis curves. Bounds deliberately use the control hull, as the JVM does.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local R=require('uml.engine.route')
local M={}
local g,m,v=G.get,G.map,E.vector
local function length(x,y) return math.sqrt(x*x+y*y) end
function M.round_corners(ps,radius)
  radius=radius or 24
  if #ps<3 then return v(ps) end; local out=v{ps[1]}
  for i=2,#ps-1 do local a,b,c=ps[i-1],ps[i],ps[i+1]; local ax,ay,bx,by=b[1]-a[1],b[2]-a[2],c[1]-b[1],c[2]-b[2]; local la,lb=length(ax,ay),length(bx,by); local r=math.min(radius,.45*la,.45*lb); local turn=0
    if la>=1e-9 and lb>=1e-9 then turn=(ax*bx+ay*by)/(la*lb) end
    if r<4 or turn>.92 then out[#out+1]=b else out[#out+1]=v{b[1]-ax/la*r,b[2]-ay/la*r}; out[#out+1]=v{b[1]+bx/lb*r,b[2]+by/lb*r} end
  end; out[#out+1]=ps[#ps]; return out
end
local function cone(dx,dy,r,p,arriving)
  local face=R.face({x=g(r,'x'),y=g(r,'y'),w=g(r,'w'),h=g(r,'h')},p); local n=({left={-1,0},right={1,0},top={0,-1},bottom={0,1}})[face]; local nx,ny=n[1],n[2]; if arriving then nx,ny=-nx,-ny end
  local l=length(dx,dy); if l<1e-6 then return nx,ny end
  local ux,uy=dx/l,dy/l; local along=ux*nx+uy*ny; local px,py=ux-along*nx,uy-along*ny; local pl=length(px,py)
  if along>0 and pl<=along then return dx,dy end
  local sx,sy;if pl<1e-9 then sx,sy=-ny,nx else sx,sy=px/pl,py/pl end; local s=1/math.sqrt(2); return l*(s*nx+s*sx),l*(s*ny+s*sy)
end
function M.basis_path(points)
  local ps=M.round_corners(points); local ops=v(); local a,b=ps[1],ps[2]
  if #ps==2 then ops[1]=m{op=E.keyword('line'),p=b}
  elseif #ps>=3 then
    ops[1]=m{op=E.keyword('line'),p=v{(5*a[1]+b[1])/6,(5*a[2]+b[2])/6}}
    for i=3,#ps do local c=ps[i]; ops[#ops+1]=m{op=E.keyword('cubic'),c1=v{(2*a[1]+b[1])/3,(2*a[2]+b[2])/3},c2=v{(a[1]+2*b[1])/3,(a[2]+2*b[2])/3},p=v{(a[1]+4*b[1]+c[1])/6,(a[2]+4*b[2]+c[2])/6}}; a,b=b,c end
    G.put(ops[#ops],'p',ps[#ps])
  end
  return m{start=ps[1] or v{0,0},ops=ops}
end
function M.constrain_ends(path,from,to)
  local out=G.copy(path); local ops=v(); for _,op in ipairs(g(path,'ops')) do ops[#ops+1]=G.copy(op) end; G.put(out,'ops',ops)
  local s=g(path,'start')
  if #ops>1 and from then local op=ops[1]; local key=g(op,'c1') and 'c1' or 'p'; local p=g(op,key); local dx,dy=cone(p[1]-s[1],p[2]-s[2],from,s,false); G.put(op,key,v{s[1]+dx,s[2]+dy}) end
  if #ops>0 and to then local op=ops[#ops]; local c=g(op,'c2'); if c then local p=g(op,'p'); local dx,dy=cone(p[1]-c[1],p[2]-c[2],to,p,true); G.put(op,'c2',v{p[1]-dx,p[2]-dy}) end end
  return out
end
function M.path_bounds(path,from,to)
  -- Retain #62's points overload for callers compiled before compose was introduced.
  if E.kind(path)~='map' then if #path==0 then return nil end; path=M.constrain_ends(M.basis_path(path),from,to) end
  local p=g(path,'start'); local rs={G.rect(p[1],p[2],0,0)}
  for _,op in ipairs(g(path,'ops')) do for _,k in ipairs{'p','c1','c2'} do local q=g(op,k); if q then rs[#rs+1]=G.rect(q[1],q[2],0,0) end end end
  return G.union(rs)
end
function M.end_tangent(path)
  local cur,behind=g(path,'start'),g(path,'start')
  for _,op in ipairs(g(path,'ops')) do behind=g(op,'c2') or cur; cur=g(op,'p') end
  return v{behind,cur}
end
function M.flatten_path(path,step)
  step=step or .0625; assert(step>0,'positive curve step required'); local cur=g(path,'start'); local out=v{cur}
  for _,op in ipairs(g(path,'ops')) do local p=g(op,'p'); local c1,c2=g(op,'c1'),g(op,'c2')
    if c1 then local t=step; while t<1.0000001 do local u=1-t; local a,b,c,d=u*u*u,3*u*u*t,3*u*t*t,t*t*t; out[#out+1]=v{a*cur[1]+b*c1[1]+c*c2[1]+d*p[1],a*cur[2]+b*c1[2]+c*c2[2]+d*p[2]}; t=t+step end else out[#out+1]=p end; cur=p
  end; return out
end
return M
