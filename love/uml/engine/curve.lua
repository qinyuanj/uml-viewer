local E=require('uml.edn')
local G=require('uml.domain.geom')
local R=require('uml.engine.route')
local M={}
local function length(x,y) return math.sqrt(x*x+y*y) end
local function round(ps)
  if #ps<3 then return ps end; local out={ps[1]}
  for i=2,#ps-1 do local a,b,c=ps[i-1],ps[i],ps[i+1]; local ax,ay,bx,by=b[1]-a[1],b[2]-a[2],c[1]-b[1],c[2]-b[2]; local la,lb=length(ax,ay),length(bx,by); local r=math.min(24,.45*la,.45*lb); local turn=la<1e-9 or lb<1e-9; turn=turn and 0 or (ax*bx+ay*by)/(la*lb)
    if r<4 or turn>.92 then out[#out+1]=b else out[#out+1]={b[1]-ax/la*r,b[2]-ay/la*r}; out[#out+1]={b[1]+bx/lb*r,b[2]+by/lb*r} end
  end; out[#out+1]=ps[#ps]; return out
end
local function cone(dx,dy,r,p,arriving)
  local face=R.face({x=G.get(r,'x'),y=G.get(r,'y'),w=G.get(r,'w'),h=G.get(r,'h')},p); local n=({left={-1,0},right={1,0},top={0,-1},bottom={0,1}})[face]; local nx,ny=n[1],n[2]; if arriving then nx,ny=-nx,-ny end
  local l=length(dx,dy); if l<1e-6 then return nx,ny end
  local ux,uy=dx/l,dy/l; local along=ux*nx+uy*ny; local px,py=ux-along*nx,uy-along*ny; local pl=length(px,py)
  if along>0 and pl<=along then return dx,dy end
  local sx,sy;if pl<1e-9 then sx,sy=-ny,nx else sx,sy=px/pl,py/pl end; local s=1/math.sqrt(2); return l*(s*nx+s*sx),l*(s*ny+s*sy)
end
function M.path_bounds(points,from,to)
  local ps=round(points); if #ps==0 then return nil end
  local ops={}; local a,b=ps[1],ps[2]
  if #ps==2 then ops[1]={p=b}
  elseif #ps>=3 then
    ops[1]={p={(5*a[1]+b[1])/6,(5*a[2]+b[2])/6}}
    for i=3,#ps do local c=ps[i]; ops[#ops+1]={c1={(2*a[1]+b[1])/3,(2*a[2]+b[2])/3},c2={(a[1]+2*b[1])/3,(a[2]+2*b[2])/3},p={(a[1]+4*b[1]+c[1])/6,(a[2]+4*b[2]+c[2])/6}}; a,b=b,c end
    ops[#ops].p=ps[#ps]
  end
  if #ops>1 and from then local op=ops[1]; local p=op.c1 or op.p; local dx,dy=cone(p[1]-ps[1][1],p[2]-ps[1][2],from,ps[1],false); local q={ps[1][1]+dx,ps[1][2]+dy}; if op.c1 then op.c1=q else op.p=q end end
  if #ops>0 and to then local op=ops[#ops]; if op.c2 then local dx,dy=cone(op.p[1]-op.c2[1],op.p[2]-op.c2[2],to,op.p,true); op.c2={op.p[1]-dx,op.p[2]-dy} end end
  local rs={G.rect(ps[1][1],ps[1][2],0,0)}; for _,op in ipairs(ops) do for _,k in ipairs{'p','c1','c2'} do local p=op[k]; if p then rs[#rs+1]=G.rect(p[1],p[2],0,0) end end end
  return G.union(rs)
end
return M
