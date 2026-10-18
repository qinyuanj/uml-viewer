-- Routing primitives needed for scene fit; full edge/hit acceptance belongs to #63.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local M={}
local g,put=G.get,G.put
local abs,min,max=math.abs,math.min,math.max
local function len(x,y) return math.sqrt(x*x+y*y) end
local function clamp(x,a,b) return max(a,min(b,x)) end
local function rect(r) return {x=g(r,'x'),y=g(r,'y'),w=g(r,'w'),h=g(r,'h')} end
local function right(r) return r.x+r.w end
local function bottom(r) return r.y+r.h end
local function cx(r) return r.x+r.w/2 end
local function cy(r) return r.y+r.h/2 end
function M.face(r,p)
  local ds={abs(p[1]-r.x),abs(p[1]-right(r)),abs(p[2]-r.y),abs(p[2]-bottom(r))}; local bi=1
  for i=2,4 do if ds[i]<ds[bi] then bi=i end end; return ({'left','right','top','bottom'})[bi]
end
local function horiz(face) return face=='left' or face=='right' end
local function port(r,face,t)
  t=clamp(t,.12,.88)
  if face=='top' then return {r.x+t*r.w,r.y} elseif face=='bottom' then return {r.x+t*r.w,bottom(r)} elseif face=='left' then return {r.x,r.y+t*r.h} else return {right(r),r.y+t*r.h} end
end
local function collapse(ps)
  local out={}
  for _,p in ipairs(ps) do local n=#out; local b,a=out[n],out[n-1]
    if not b or p[1]~=b[1] or p[2]~=b[2] then
      if a and ((abs(a[1]-b[1])<.51 and abs(b[1]-p[1])<.51) or (abs(a[2]-b[2])<.51 and abs(b[2]-p[2])<.51)) and b[1]>=min(a[1],p[1]) and b[1]<=max(a[1],p[1]) and b[2]>=min(a[2],p[2]) and b[2]<=max(a[2],p[2]) then
        out[n]=nil; if a[1]~=p[1] or a[2]~=p[2] then out[#out+1]=p end
      else out[#out+1]=p end
    end
  end
  return out
end
local function flow(a,b,lr)
  if lr then if cx(a)<=cx(b) then return 'right','left' end; return 'left','right' end
  if cy(a)<=cy(b) then return 'bottom','top' end; return 'top','bottom'
end
local function union(a,b) local x,y=min(a.x,b.x),min(a.y,b.y); return {x=x,y=y,w=max(right(a),right(b))-x,h=max(bottom(a),bottom(b))-y} end
local function via_track(s,e,track,lr) if lr then return collapse{s,{track,s[2]},{track,e[2]},e} end; return collapse{s,{s[1],track},{e[1],track},e} end
local function rank_box(cs,rank) local bb; for _,c in ipairs(cs) do if c.rank==rank then bb=bb and union(bb,c.r) or c.r end end; return bb end
local function candidates(a,b,ft,tt,i,n,cs,lr)
  local ar,br=a.r,b.r
  local stacked=lr and (br.x-right(ar)>8 or ar.x-right(br)>8) or (not lr and (br.y-bottom(ar)>8 or ar.y-bottom(br)>8))
  local result={}; local function add(p) if p then result[#result+1]=p end end
  if not stacked then
    local ff,tf
    if lr then if cy(ar)<cy(br) then ff,tf='bottom','top' else ff,tf='top','bottom' end else if cx(ar)<cx(br) then ff,tf='right','left' else ff,tf='left','right' end end
    local s,e=port(ar,ff,ft),port(br,tf,tt)
    local lo,hi=lr and max(ar.x,br.x) or max(ar.y,br.y),lr and min(right(ar),right(br)) or min(bottom(ar),bottom(br))
    if lo+1<hi then if lr then add(collapse{{lo+ft*(hi-lo),s[2]},{lo+tt*(hi-lo),e[2]}}) else add(collapse{{s[1],lo+ft*(hi-lo)},{e[1],lo+tt*(hi-lo)}}) end
    elseif lr then add(collapse{s,{s[1],(s[2]+e[2])/2},{e[1],(s[2]+e[2])/2},e}) else add(via_track(s,e,(s[1]+e[1])/2,true)) end
    local bb=union(ar,br); local outer=lr and 'right' or 'bottom'; local inner=lr and 'left' or 'top'
    for _,f in ipairs{outer,inner} do local ps,pe=port(ar,f,ft),port(br,f,tt); local pad=10+i*10; local track
      if f=='right' then track=right(bb)+pad elseif f=='left' then track=bb.x-pad elseif f=='bottom' then track=bottom(bb)+pad else track=bb.y-pad end
      add(via_track(ps,pe,track,lr))
    end
    bb=rank_box(cs,a.rank) or ar; add(via_track(port(ar,outer,ft),port(br,outer,tt),(lr and right(bb) or bottom(bb))+20+i*10,lr))
  else
    local ff,tf=flow(ar,br,lr); local s,e=port(ar,ff,ft),port(br,tf,tt)
    local lo=lr and min(right(ar),right(br)) or min(bottom(ar),bottom(br)); local hi=lr and max(ar.x,br.x) or max(ar.y,br.y)
    local step=max(10,hi-lo)/(max(n,1)+1)
    add(via_track(s,e,lo+(i+1)*step,lr))
    local pad=20+i*10
    if lr then
      local top,bot=min(ar.y,br.y)-pad,max(bottom(ar),bottom(br))+pad
      local st,sb=port(ar,'top',ft),port(ar,'bottom',ft)
      add(collapse{st,{st[1],top},{e[1],top},e}); add(collapse{sb,{sb[1],bot},{e[1],bot},e})
    else
      local left,rgt=min(ar.x,br.x)-pad,max(right(ar),right(br))+pad; local sl,sr=port(ar,'left',ft),port(ar,'right',ft)
      add(collapse{sl,{left,sl[2]},{left,e[2]},e}); add(collapse{sr,{rgt,sr[2]},{rgt,e[2]},e})
    end
    local hops={s}; local dir=a.rank<b.rank and 1 or -1; local rank=a.rank+dir
    while (dir==1 and rank<b.rank) or (dir==-1 and rank>b.rank) do
      local bb=rank_box(cs,rank)
      if bb then if lr then local y=clamp((s[2]+e[2])/2,min(s[2],e[2]),max(s[2],e[2])); hops[#hops+1]={bb.x-10-i*10,y}; hops[#hops+1]={right(bb)+10+i*10,y}
        else local x=clamp((s[1]+e[1])/2,min(s[1],e[1]),max(s[1],e[1])); local top=clamp(bb.y-10-i*10,min(s[2],e[2]),max(s[2],e[2])); local bot=clamp(bottom(bb)+10+i*10,min(s[2],e[2]),max(s[2],e[2])); hops[#hops+1]={x,top}; if abs(top-bot)>=1 then hops[#hops+1]={x,bot} end end end
      rank=rank+dir
    end
    if #hops>1 then hops[#hops+1]=e; add(collapse(hops)) end
    add(via_track(s,e,lo+(n-i)*step,lr)); add(collapse{s,e})
  end
  return result
end
local function reverses(ps)
  if #ps<3 then return false end; local s,e=ps[1],ps[#ps]; local axis=abs(e[1]-s[1])>=abs(e[2]-s[2]) and 1 or 2; local dir=e[axis]-s[axis]
  for i=1,#ps-1 do local a,b=ps[i][axis],ps[i+1][axis]; if (dir>0 and a>e[axis]+12 and b<a-1) or (dir<0 and a<e[axis]-12 and b>a+1) then return true end end; return false
end
local function hit(ps,a,b,cs)
  for i=1,#ps-1 do local p,q=ps[i],ps[i+1]; for _,c in ipairs(cs) do if c~=a and c~=b and not c.dummy then local r=c.r; if min(p[1],q[1])<right(r) and max(p[1],q[1])>r.x and min(p[2],q[2])<bottom(r) and max(p[2],q[2])>r.y then return true end end end end; return false
end
local function stub(r,p,q) local h=horiz(M.face(r,p)); local dx,dy=abs(q[1]-p[1]),abs(q[2]-p[2]); if h then return dx+1>=dy end; return dy+1>=dx end
local function score(ps)
  local best=1
  for i=2,#ps-1 do local a,b,c=ps[i-1],ps[i],ps[i+1]; local x,y=b[1]-a[1],b[2]-a[2]; local u,v=c[1]-b[1],c[2]-b[2]; local la,lb=len(x,y),len(u,v); if la>=1e-9 and lb>=1e-9 then best=min(best,(x*u+y*v)/(la*lb)) end end; return best
end
local function choose(ps,a,b,cs)
  local clear,first_ok,best,bestscore={},nil,nil,-math.huge
  for _,p in ipairs(ps) do if #p>0 and not hit(p,a,b,cs) then clear[#clear+1]=p; if not reverses(p) then first_ok=first_ok or p; if #p>1 and stub(a.r,p[1],p[2]) and stub(b.r,p[#p],p[#p-1]) then local sc=score(p); if sc>=bestscore then best,bestscore=p,sc end end end end end
  return best or first_ok or clear[1] or ps[1]
end
local function sync(ps,start,p,face)
  local n=#ps; local at=start and 1 or n; ps[at]=p
  if n>=3 then local i=start and 2 or n-1; local inner=ps[i]; inner=horiz(face) and {inner[1],p[2]} or {p[1],inner[2]}; local dx,dy=inner[1]-p[1],inner[2]-p[2]; local l=len(dx,dy); if l>=1e-9 then local scale=max(1,20/l); inner={p[1]+dx*scale,p[2]+dy*scale} end; ps[i]=inner end
end
local function slide(r,p,q)
  local f=M.face(r,p); local x,y=p[1],p[2]
  if horiz(f) then local lim=max(1,abs(x-q[1])); y=clamp(max(q[2]-lim,min(q[2]+lim,y)),r.y+.12*r.h,bottom(r)-.12*r.h)
  else local lim=max(1,abs(y-q[2])); x=clamp(max(q[1]-lim,min(q[1]+lim,x)),r.x+.12*r.w,right(r)-.12*r.w) end; return {x,y}
end
local function stable_sort(xs,scorefn)
  local decorated={}; for i,x in ipairs(xs) do decorated[i]={x=x,i=i,s=scorefn(x)} end
  table.sort(decorated,function(a,b) if a.s==b.s then return a.i<b.i end; return a.s<b.s end)
  for i,x in ipairs(decorated) do xs[i]=x.x end
end
function M.route(scene)
  local cs,idx={},{}; local lr=G.name(g(g(scene,'diagram'),'direction')); lr=lr=='lr' or lr=='rl'
  for _,c in ipairs(g(scene,'classes')) do local r=g(c,'rect'); if type((g(r,'x')))=='number' and type((g(r,'y')))=='number' then local x={r=rect(r),id=E.encode(g(c,'id')),rank=g(c,'rank') or 0,dummy=g(c,'dummy?')}; cs[#cs+1]=x; idx[x.id]=x end end
  local es,from,to,hops={},{},{},{}
  for _,e in ipairs(g(g(scene,'diagram'),'edges') or {}) do local f,t=idx[E.encode(g(e,'from'))],idx[E.encode(g(e,'to'))]; if f and t then
    local x={edge=e,a=f,b=t}; es[#es+1]=x; from[f.id]=from[f.id] or {}; from[f.id][#from[f.id]+1]=x; to[t.id]=to[t.id] or {}; to[t.id][#to[t.id]+1]=x
    local k=min(f.rank,t.rank)..':'..max(f.rank,t.rank); hops[k]=hops[k] or {}; hops[k][#hops[k]+1]=x
  end end
  for _,group in pairs(from) do stable_sort(group,function(x) return lr and cy(x.b.r) or cx(x.b.r) end); for i,x in ipairs(group) do x.ft=i/(#group+1) end end
  for _,group in pairs(to) do stable_sort(group,function(x) return lr and cy(x.a.r) or cx(x.a.r) end); for i,x in ipairs(group) do x.tt=i/(#group+1) end end
  for _,group in pairs(hops) do table.sort(group,function(a,b) if a.a.id==b.a.id then return a.b.id<b.b.id end; return a.a.id<b.a.id end); for i,x in ipairs(group) do x.i=i-1; x.n=#group end end
  local atts={}
  for _,x in ipairs(es) do local ps=choose(candidates(x.a,x.b,x.ft,x.tt,x.i,x.n,cs,lr),x.a,x.b,cs)
    local s,e=slide(x.a.r,ps[1],ps[2]),slide(x.b.r,ps[#ps],ps[#ps-1]); sync(ps,true,s,M.face(x.a.r,s)); sync(ps,false,e,M.face(x.b.r,e)); x.ps=ps
    for _,start in ipairs{true,false} do local c=start and x.a or x.b; local p=start and ps[1] or ps[#ps]; local face=M.face(c.r,p); local k=c.id..face; atts[k]=atts[k] or {}; atts[k][#atts[k]+1]={x=x,start=start,c=c,p=p,face=face} end
  end
  for _,group in pairs(atts) do if #group>1 then
    local r,f=group[1].c.r,group[1].face; stable_sort(group,function(a) return horiz(f) and a.p[2] or a.p[1] end)
    local lo,hi=horiz(f) and r.y+.12*r.h or r.x+.12*r.w,horiz(f) and bottom(r)-.12*r.h or right(r)-.12*r.w
    for i,a in ipairs(group) do local v=lo+i*(hi-lo)/(#group+1); local p=horiz(f) and {a.p[1],v} or {v,a.p[2]}; sync(a.x.ps,a.start,p,f) end
  end end
  local edges=E.vector(); for _,x in ipairs(es) do local e=G.copy(x.edge); local ps=E.vector(); for _,p in ipairs(x.ps) do ps[#ps+1]=E.vector(p) end; put(e,'points',ps); put(e,'head',E.keyword(({inheritance='triangle',implements='triangle',composition='diamond-fill',aggregation='diamond'})[G.name(g(e,'kind'))] or 'open')); edges[#edges+1]=e end; put(scene,'edges',edges); return scene
end
return M
