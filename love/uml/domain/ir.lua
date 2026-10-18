local E=require('uml.edn')
local G=require('uml.domain.geom')
local M={}
local g,m,put=G.get,G.map,G.put
local V,K=E.vector,E.keyword
local function id(x)
  if E.kind(x)=='keyword' then return x end
  if type(x)=='string' then return K(x:lower():gsub('%s+','-')) end
  if E.kind(x)=='symbol' then return K(G.name(x)) end
  error('id must be a keyword or string')
end
local function number(x,kind)
  if x==nil then return E.nil_value end
  assert(type(x)=='number',kind..' must be a number')
  if kind=='coverage' then assert(x>=0 and x<=1,'coverage must be between 0 and 1')
  else assert(x>=0,kind..' must be non-negative'); if kind~='cc' then assert(x==math.floor(x),kind..' must be an integer') end end
  return x
end
local function crap(x)
  if x==nil then return E.nil_value end
  if type(x)=='number' then return m{mu=x,max=x,sigma=0} end
  assert(E.kind(x)=='map','crap must be a number or map')
  local r=m{mu=g(x,'mu') or E.nil_value,max=g(x,'max') or g(x,'mu') or 0,sigma=g(x,'sigma') or g(x,'sd') or 0}; if g(x,'n') and g(x,'n')>0 then put(r,'n',math.floor(g(x,'n'))) end; return r
end
local function member(x)
  if type(x)=='string' then return m{text=x} end
  assert(E.kind(x)=='map','member must be a string or map')
  local text=g(x,'text'); if not text then
    text=g(x,'name') or ''; local args=g(x,'args'); if args and #args>0 then text=text..'('..table.concat(args,', ')..')' end
    if g(x,'type') then text=text..' : '..g(x,'type') end; if g(x,'returns') then text=text..' : '..g(x,'returns') end
  end
  local r=m{text=text}; for _,k in ipairs{'coverage','killed','survived','uncovered','sites','cc','crap'} do local raw,found=E.get(x,K(k)); if found then put(r,k,k=='crap' and crap(g(x,k)) or number(g(x,k),k)) end end
  if g(x,'private')==true then put(r,'private',true) end; put(r,'name',g(x,'name')); return r
end
local function class(c)
  local label=g(c,'name') or G.name(g(c,'id')); assert(label,'class needs :name or :id')
  local r=m{id=id(g(c,'id') or label),name=label,shape=(g(c,'foreign') or G.name(g(c,'shape'))=='oval') and K('oval') or E.nil_value,stereotype=g(c,'stereotype') or E.nil_value,crap=crap(g(c,'crap')),['hide-members']=g(c,'hide-members')==true}
  for _,k in ipairs{'coverage','cc','killed','survived','uncovered','sites'} do put(r,k,number(g(c,k),k)) end
  for _,k in ipairs{'fields','ops'} do local xs=V(); for _,x in ipairs(g(c,k) or {}) do xs[#xs+1]=member(x) end; put(r,k,xs) end
  for _,k in ipairs{'ns','lang','file','level'} do put(r,k,g(c,k)) end
  return r
end
function M.normalize(raw)
  local ps,fs,es=V(),V(),V(); local ids={}
  local function add(c) local x=class(c); local k=E.encode(g(x,'id')); assert(not ids[k],'duplicate class id: '..k); ids[k]=true; return x end
  for _,p in ipairs(g(raw,'packages') or {}) do local label=g(p,'label') or G.name(g(p,'id')); assert(label,'package needs :label or :id'); local cs=V(); for _,c in ipairs(g(p,'classes') or {}) do cs[#cs+1]=add(c) end; ps[#ps+1]=m{id=id(g(p,'id') or label),label=label,crap=crap(g(p,'crap')),classes=cs} end
  for _,c in ipairs(g(raw,'foreign') or {}) do fs[#fs+1]=add(c) end
  for _,e in ipairs(g(raw,'edges') or {}) do assert(g(e,'from') and g(e,'to'),'edge needs :from and :to'); local f,t=id(g(e,'from')),id(g(e,'to')); assert(ids[E.encode(f)],'edge :from unknown class'); assert(ids[E.encode(t)],'edge :to unknown class'); local x=m{from=f,to=t,kind=id(g(e,'kind') or K('association')),label=g(e,'label') or E.nil_value}; if g(e,'violating')==true then put(x,'violating',true) end; es[#es+1]=x end
  return m{title=g(raw,'title') or 'UML',direction=id(g(raw,'direction') or K('tb')),packages=ps,foreign=fs,edges=es}
end
function M.read_document(text)
  local raw=E.parse(text); assert(E.kind(raw)=='map','document must be an EDN map')
  if g(raw,'hierarchical') then return raw end
  local ds=V(); for _,d in ipairs(g(raw,'diagrams') or V{raw}) do ds[#ds+1]=M.normalize(d) end; return m{title=g(raw,'title') or 'UML',diagrams=ds}
end
return M
