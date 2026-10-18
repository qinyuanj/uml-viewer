local E=require('uml.edn')
local G=require('uml.domain.geom')
local C=require('uml.domain.config')
local M={}
local g,m,put,copy,name=G.get,G.map,G.put,G.copy,G.name
local V,K=E.vector,E.keyword
local function seq(x) return x or {} end
local function key(x) return E.encode(x) end
-- JVM String.count measures UTF-16 units, including surrogate pairs.
function M.text_w(s)
  local n=0; for ch in (s or ''):gmatch('[%z\1-\127\194-\244][\128-\191]*') do n=n+(#ch==4 and 2 or 1) end; return 8*n
end
function M.class_lines(c)
  local lines=V(); local function add(kind,text,extra) local l=m{kind=K(kind),text=text or E.nil_value}; for _,e in ipairs(extra or {}) do E.assoc(l,e[1],e[2]) end; lines[#lines+1]=l end
  if g(c,'stereotype') then add('stereo','«'..name(g(c,'stereotype'))..'»') end
  add('name',g(c,'name'))
  if not g(c,'hide-members') then
    local kids=seq(g(c,'contents'))
    if #kids>0 then add('rule'); for _,ch in ipairs(kids) do add('child',g(ch,'name'),m{id=g(ch,'id'),['drill?']=g(ch,'drill?')==true}) end
    else
      for _,kind in ipairs{'fields','ops'} do local shown={}; for _,x in ipairs(seq(g(c,kind))) do if kind~='ops' or not g(x,'private') then shown[#shown+1]=x end end
        if #shown>0 then add('rule'); for _,x in ipairs(shown) do add(kind=='fields' and 'field' or 'op',g(x,'text')) end end
      end
    end
  end
  return lines
end
function M.class_box_size(c)
  local lines=M.class_lines(c); local w,h=120,24
  for _,l in ipairs(lines) do if name(g(l,'kind'))=='rule' then h=h+12 else h=h+18; w=math.max(w,24+M.text_w(g(l,'text'))) end end
  return w,h,lines
end
local function port_w(d) return math.max(44,10+M.text_w(g(d,'name') or name(g(d,'id')))) end
local function pack(c)
  c=copy(c); local w,h,lines=M.class_box_size(c); local ins,outs=seq(g(c,'in-deps')),seq(g(c,'out-deps')); local iw,ow=0,0
  for i,d in ipairs(ins) do iw=iw+port_w(d)+(i>1 and 6 or 0) end
  for _,d in ipairs(outs) do ow=math.max(ow,port_w(d)) end; if #outs>=2 then ow=ow+16 end
  local isp=#ins>0 and 40 or 0; local osp=#outs>0 and 58+(#outs-1)*28 or 0
  for k,v in pairs{['content-w']=w,['content-h']=h,['in-space']=isp,['out-space']=osp,w=math.max(w,iw,ow),h=isp+h+osp,lines=lines} do put(c,k,v) end
  return c
end
local function ports(c)
  local r=g(c,'rect'); local ins,outs=seq(g(c,'in-deps')),seq(g(c,'out-deps')); local ip,op=V(),V(); local total=0
  for i,d in ipairs(ins) do total=total+port_w(d)+(i>1 and 6 or 0) end
  local x=G.cx(r)-total/2
  for _,d in ipairs(ins) do local p=copy(d); put(p,'rect',G.rect(x,g(r,'y')-40,port_w(d),18)); ip[#ip+1]=p; x=x+port_w(d)+6 end
  local widest=0; for _,d in ipairs(outs) do widest=math.max(widest,port_w(d)) end
  x=G.cx(r)-(widest+(#outs>=2 and 16 or 0))/2
  for i,d in ipairs(outs) do local p=copy(d); put(p,'rect',G.rect(x+((i-1)%2)*16,G.bottom(r)+40+(i-1)*28,port_w(d),18)); op[#op+1]=p end
  put(c,'in-ports',ip); put(c,'out-ports',op); return c
end
local function place(classes,edges,direction)
  local cs,ids,idx,out,high,nbr={},{},{},{},{},{}
  for i,c in ipairs(classes) do c=pack(c); cs[i]=c; local id=key(g(c,'id')); ids[i]=id; idx[id]=c; out[id]={}; nbr[id]={} end
  for _,e in ipairs(edges) do local f,t=key(g(e,'from')),key(g(e,'to')); if idx[f] and idx[t] then
    nbr[f][t]=true; nbr[t][f]=true
    local k=name(g(e,'kind')); if k=='inheritance' or k=='implements' then f,t=t,f end; out[f][#out[f]+1]=t; high[t]=true
  end end
  local queue,ranks={},{}; for _,id in ipairs(ids) do if not high[id] then queue[#queue+1]=id; ranks[id]=0 end end
  if #queue==0 then for _,id in ipairs(ids) do queue[#queue+1]=id; ranks[id]=0 end end
  local qi=1; while qi<=#queue do local id=queue[qi]; qi=qi+1; for _,t in ipairs(out[id]) do if ranks[t]==nil then ranks[t]=ranks[id]+1; queue[#queue+1]=t end end end
  local horizontal=direction=='lr' or direction=='rl'; local levels=false
  for _,c in ipairs(cs) do if g(c,'level')~=nil then levels=true end end; levels=levels and not horizontal
  local groups,keys,pos={},{},{}
  for i,id in ipairs(ids) do local r=levels and (g(idx[id],'level') or 0) or (ranks[id] or 0); if not groups[r] then groups[r]={}; keys[#keys+1]=r end; groups[r][#groups[r]+1]=id; pos[id]=(i-1)*80 end
  table.sort(keys,function(a,b) if levels then return a>b end; return a<b end)
  local function ordered(xs)
    local list={}; local function score(id) local sum,n=0,0; for t in pairs(nbr[id]) do if pos[t] then sum=sum+pos[t]; n=n+1 end end; return n>0 and sum/n or pos[id] or 0 end
    for i,id in ipairs(xs) do list[i]={id=id,index=i,score=score(id)} end
    table.sort(list,function(a,b) if a.score==b.score then return a.index<b.index end; return a.score<b.score end)
    local r={}; for i,x in ipairs(list) do r[i]=x.id end; return r
  end
  for _=1,5 do for _,r in ipairs(keys) do for i,id in ipairs(ordered(groups[r])) do pos[id]=(i-1)*80 end end end
  local gs,total_h={},0
  for _,r in ipairs(keys) do local xs=ordered(groups[r]); local w,h,colh=0,0,0; for i,id in ipairs(xs) do w=math.max(w,g(idx[id],'w')); h=math.max(h,g(idx[id],'h')); colh=colh+g(idx[id],'h')+(i>1 and 40 or 0) end; gs[#gs+1]={rank=r,ids=xs,w=w,h=h,colh=colh}; total_h=math.max(total_h,colh) end
  local result=V(); local along=0
  for _,group in ipairs(gs) do local across=horizontal and (total_h-group.colh)/2 or 0
    for _,id in ipairs(group.ids) do local c=idx[id]; local x,y=horizontal and along or across,horizontal and across or along
      put(c,'rect',G.rect(x+(g(c,'w')-g(c,'content-w'))/2,y+g(c,'in-space'),g(c,'content-w'),g(c,'content-h'))); put(c,'rank',group.rank); result[#result+1]=c
      across=across+(horizontal and g(c,'h') or g(c,'w'))+40
    end
    along=along+(horizontal and group.w or group.h)+140
  end
  return result
end
function M.layout(diagram)
  local pkgs,classes=V(),V(); local y=40; local stride=2
  for _,p in ipairs(seq(g(diagram,'packages'))) do stride=math.max(stride,1+#seq(g(p,'classes'))) end
  for i,p in ipairs(seq(g(diagram,'packages'))) do
    local inner=place(seq(g(p,'classes')),seq(g(diagram,'edges')),name(g(diagram,'direction')) or 'tb'); local rects={}; local real=V()
    for _,c in ipairs(inner) do put(c,'rect',G.move(g(c,'rect'),52,y+44)); put(c,'package',g(p,'id')); put(c,'rank',(i-1)*stride+g(c,'rank')); ports(c)
      if not g(c,'dummy?') then real[#real+1]=c; rects[#rects+1]=g(c,'rect'); for _,k in ipairs{'in-ports','out-ports'} do for _,port in ipairs(g(c,k)) do rects[#rects+1]=g(port,'rect') end end end
    end
    local body=G.union(rects) or G.rect(52,y+44,160,40); local pr=G.rect(40,y,math.max(G.right(body)+12-40,24+M.text_w(g(p,'label')),180),G.bottom(body)+12-y)
    local pkg=m{id=g(p,'id'),label=g(p,'label'),title=g(p,'label'),rect=pr}; local source=#real>0 and real or inner; put(pkg,'crap',C.pool_crap(source,true)); for _,e in ipairs(C.pool_mutants(source) or {}) do E.assoc(pkg,e[1],e[2]) end
    pkgs[#pkgs+1]=pkg; for _,c in ipairs(inner) do if g(c,'dummy?') then put(c,'rect',pr); ports(c) end; classes[#classes+1]=c end; y=y+g(pr,'h')+80
  end
  local rs={}; for _,p in ipairs(pkgs) do rs[#rs+1]=g(p,'rect') end; local bounds=G.union(rs) or G.rect(40,40,400,300); local ovals=V(); local oy=g(bounds,'y')
  for _,c in ipairs(seq(g(diagram,'foreign'))) do c=copy(c); local w=math.max(80,24+M.text_w(g(c,'name'))); local h=math.max(44,math.floor(.55*w)); put(c,'shape',K('oval')); put(c,'lines',V{m{kind=K('name'),text=g(c,'name')}}); put(c,'rect',G.rect(G.right(bounds)+40,oy,w,h)); ovals[#ovals+1]=c; oy=oy+h+40 end
  local oh=#ovals>0 and oy-40-g(bounds,'y') or 0; local dy=#ovals>0 and math.max(0,(g(bounds,'h')-oh)/2) or 0
  for _,c in ipairs(ovals) do put(c,'rect',G.move(g(c,'rect'),0,dy)); classes[#classes+1]=c end
  return m{diagram=diagram,packages=pkgs,classes=classes,edges=V()}
end
return M
