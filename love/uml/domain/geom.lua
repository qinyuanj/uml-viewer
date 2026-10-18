-- Rectangles and points retain the public EDN protocol at geometry boundaries.
local E=require('uml.edn')
local M={}
function M.get(m,k) if E.kind(m)=='map' then local v=E.get(m,E.keyword(k)); if v~=E.nil_value then return v end end end
function M.map(t) local m=E.map(); for k,v in pairs(t) do E.assoc(m,E.keyword(k),v) end; return m end
function M.copy(m) local r=E.map(); for _,v in ipairs(m or {}) do r[#r+1]={v[1],v[2]} end; return r end
function M.put(m,k,v) if v~=nil then E.assoc(m,E.keyword(k),v) end; return m end
function M.name(x) if type(x)=='string' then return x end; return x and x.name:match('[^/]+$') end
local g=M.get
function M.rect(x,y,w,h) return M.map{x=x,y=y,w=w,h=h} end
function M.right(r) return g(r,'x')+g(r,'w') end
function M.bottom(r) return g(r,'y')+g(r,'h') end
function M.cx(r) return g(r,'x')+g(r,'w')/2 end
function M.cy(r) return g(r,'y')+g(r,'h')/2 end
function M.move(r,x,y) return M.rect(g(r,'x')+x,g(r,'y')+y,g(r,'w'),g(r,'h')) end
function M.inflate(r,p) return M.rect(g(r,'x')-p,g(r,'y')-p,g(r,'w')+2*p,g(r,'h')+2*p) end
function M.union(rs)
  local x,y,r,b
  for _,v in ipairs(rs) do if v then x=x and math.min(x,g(v,'x')) or g(v,'x'); y=y and math.min(y,g(v,'y')) or g(v,'y'); r=r and math.max(r,M.right(v)) or M.right(v); b=b and math.max(b,M.bottom(v)) or M.bottom(v) end end
  if x then return M.rect(x,y,r-x,b-y) end
end
function M.contains(r,x,y) return r and x>=g(r,'x') and x<=M.right(r) and y>=g(r,'y') and y<=M.bottom(r) end
function M.overlaps(a,b) return g(a,'x')<M.right(b) and M.right(a)>g(b,'x') and g(a,'y')<M.bottom(b) and M.bottom(a)>g(b,'y') end
function M.point_seg_dist(p,a,b)
  local dx,dy=b[1]-a[1],b[2]-a[2]; local l=dx*dx+dy*dy; local t=0
  if l>=1e-12 then t=math.max(0,math.min(1,((p[1]-a[1])*dx+(p[2]-a[2])*dy)/l)) end
  local x,y=p[1]-a[1]-t*dx,p[2]-a[2]-t*dy; return math.sqrt(x*x+y*y)
end
function M.near_polyline(p,ps,pad) for i=1,#ps-1 do if M.point_seg_dist(p,ps[i],ps[i+1])<=pad then return true end end; return false end
function M.near_triangle(p,ps,pad)
  local function sign(a,b) return (p[1]-b[1])*(a[2]-b[2])-(a[1]-b[1])*(p[2]-b[2]) end
  local a,b,c=sign(ps[1],ps[2]),sign(ps[2],ps[3]),sign(ps[3],ps[1])
  return not ((a<0 or b<0 or c<0) and (a>0 or b>0 or c>0)) or M.near_polyline(p,{ps[1],ps[2],ps[3],ps[1]},pad)
end
local function near(a,b) return M.point_seg_dist(a,b,b)<.51 end
local function lerp(a,b,t) return E.vector{a[1]+t*(b[1]-a[1]),a[2]+t*(b[2]-a[2])} end
function M.gap_segment(a,b,rects,pad)
  local out=E.vector(); if near(a,b) then return out end
  local removed={}
  for _,r0 in ipairs(rects) do local r=M.inflate(r0,pad); local dx,dy=b[1]-a[1],b[2]-a[2]; local lo,hi=0,1; local valid=true
    for _,pq in ipairs{{-dx,a[1]-g(r,'x')},{dx,M.right(r)-a[1]},{-dy,a[2]-g(r,'y')},{dy,M.bottom(r)-a[2]}} do
      local p,q=pq[1],pq[2]
      if math.abs(p)<1e-12 then if q<0 then valid=false; break end
      else local t=q/p; if p<0 then if t>hi then valid=false; break end; lo=math.max(lo,t) else if t<lo then valid=false; break end; hi=math.min(hi,t) end end
    end
    if valid and lo<hi then removed[#removed+1]={lo,hi} end
  end
  table.sort(removed,function(x,y) return x[1]<y[1] end); local merged={}
  for _,iv in ipairs(removed) do local last=merged[#merged]; if last and iv[1]<=last[2]+1e-9 then last[2]=math.max(last[2],iv[2]) else merged[#merged+1]=iv end end
  local function keep(lo,hi) local p,q=lerp(a,b,lo),lerp(a,b,hi); if not near(p,q) then out[#out+1]=E.vector{p,q} end end
  local u=0; for _,iv in ipairs(merged) do if u<iv[1]-1e-9 then keep(u,iv[1]) end; u=math.max(u,iv[2]) end
  if u<.999999 then keep(u,1) end; return out
end
function M.gap_polyline(ps,rects,pad)
  local out=E.vector(); if #ps<2 then return out end; if #rects==0 then return E.vector{ps} end
  for i=1,#ps-1 do for _,piece in ipairs(M.gap_segment(ps[i],ps[i+1],rects,pad)) do local last=out[#out]; if last and near(last[#last],piece[1]) then last[#last+1]=piece[2] else out[#out+1]=piece end end end; return out
end
return M
