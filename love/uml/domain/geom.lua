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
return M
