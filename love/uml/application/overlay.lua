-- Pure metrics overlay. Snapshot records contain path, text, mtime and size;
-- all filesystem/native operations belong to the metrics adapter.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local C=require('uml.domain.config')
local M={}
local g,m,put=G.get,G.map,G.put
local function copy(x) return E.parse(E.encode(x)) end
function M.load(files)
  local crap,mutate={},{}
  local ordered={}; for _,f in ipairs(files or {}) do ordered[#ordered+1]=f end
  assert(type(files)=='table','snapshots must be an array of records, not a filesystem root')
  local function time(f)
    if type(f.mtime)=='number' then return f.mtime,0 end
    local low,high=tostring(f.mtime):match('^(%d+):(%d+)$')
    if low then return tonumber(high),tonumber(low) end
    return tonumber(f.mtime) or 0,0
  end
  table.sort(ordered,function(a,b)
    local ah,al=time(a); local bh,bl=time(b)
    if ah~=bh then return ah<bh end; if al~=bl then return al<bl end; return a.path<b.path
  end)
  for _,f in ipairs(ordered) do
    local data=E.parse(f.text)
    do -- paths are relative to the project root
      if f.path:gsub('\\','/'):match('%.metrics/crap%.edn$') then
        for _,entry in ipairs(g(data,'entries') or {}) do local ns=g(entry,'namespace'); if ns then crap[ns]=crap[ns] or {}; crap[ns][#crap[ns]+1]=entry end end
      else
        local ns=g(data,'namespace'); local source=g(data,'source')
        if not ns and source then ns=source:gsub('\\','/'):gsub('^src/',''):gsub('%.[^%.]+$',''):gsub('/','.') end
        if ns then mutate[ns]=data end
      end
    end
  end
  return {crap=crap,mutate=mutate}
end
local function doc_classes(doc)
  local cs={}
  if g(doc,'hierarchical') then return g(doc,'classes') or {} end
  for _,d in ipairs(g(doc,'diagrams') or {doc}) do for _,p in ipairs(g(d,'packages') or {}) do for _,c in ipairs(g(p,'classes') or {}) do cs[#cs+1]=c end end end
  return cs
end
local function owner(classes,prefix,ns)
  ns=ns:gsub('::','.')
  local root,count=nil,0
  for _,c in ipairs(classes) do if G.name(g(c,'lang'))=='rust' and not g(c,'foreign') and not G.name(g(c,'id')):find('.',1,true) then root=c; count=count+1 end end
  local best,rank,length
  for _,c in ipairs(classes) do if not g(c,'foreign') and G.name(g(c,'shape'))~='oval' then
    local id=G.name(g(c,'id')); local r,l
    if g(c,'ns')==ns then r,l=0,0 elseif id==ns then r,l=1,0
    elseif id and ns:sub(1,#id+1)==id..'.' then r,l=2,-#id
    elseif count==1 and c==root and prefix==ns then r,l=3,0 end
    if r and (not rank or r<rank or (r==rank and l<length)) then best,rank,length=c,r,l end
  end end
  return best
end
local function sites(f) local n=g(f,'sites'); if n~=nil then return n end; return (g(f,'killed') or 0)+(g(f,'survived') or 0)+(g(f,'uncovered') or 0) end
local function paint(c,nss,metrics)
  local entries,forms,form_order,by_name={},{},{},{}
  for _,ns in ipairs(nss) do
    for _,e in ipairs(metrics.crap[ns] or {}) do entries[#entries+1]=e; by_name[g(e,'name')]=e end
    for _,f in ipairs(g(metrics.mutate[ns],'forms') or {}) do
      local id=g(f,'id') or ''; local private=id:sub(1,6)=='defn-/'
      local name=private and id:sub(7) or id:sub(1,5)=='defn/' and id:sub(6)
      if name then if not forms[name] then form_order[#form_order+1]=name end; forms[name]={form=f,private=private} end
    end
  end
  local names,seen,existing={},{},{}
  local function add(n) if n and not seen[n] then names[#names+1]=n; seen[n]=true end end
  for _,op in ipairs(g(c,'ops') or {}) do local n=g(op,'name'); add(n); if n and not existing[n] then existing[n]=op end end
  for _,e in ipairs(entries) do add(g(e,'name')) end; for _,n in ipairs(form_order) do add(n) end
  local ops=E.vector()
  -- Keep unnamed authored member lines as well as the named union.
  for _,op in ipairs(g(c,'ops') or {}) do if not g(op,'name') then ops[#ops+1]=op end end
  for _,n in ipairs(names) do
    local op=existing[n] or m{name=n,text=n}; if not g(op,'text') then put(op,'text',n) end
    local e,f=by_name[n],forms[n]
    if e then put(op,'cc',g(e,'complexity') or E.nil_value); put(op,'crap',g(e,'crap') or E.nil_value); local cov=g(e,'coverage'); put(op,'coverage',cov and cov/100 or E.nil_value) end
    if f then for _,k in ipairs{'killed','survived','uncovered'} do put(op,k,g(f.form,k) or 0) end; put(op,'sites',sites(f.form)); if f.private then put(op,'private',true) end end
    ops[#ops+1]=op
  end
  if #ops>0 then put(c,'ops',ops) end
  local scores,cov,cc={}, {},0
  for _,e in ipairs(entries) do local s=g(e,'crap'); if s~=nil then scores[#scores+1]=s end; local v=g(e,'coverage'); if v~=nil then cov[#cov+1]=v end; cc=cc+(g(e,'complexity') or 0) end
  if #scores>0 then put(c,'crap',C.summarize_scores(scores)); put(c,'cc',cc) end
  if #cov>0 then local sum=0; for _,v in ipairs(cov) do sum=sum+v end; put(c,'coverage',sum/#cov/100) end
  if #form_order>0 then
    local sums={killed=0,survived=0,uncovered=0,sites=0}
    for _,f in pairs(forms) do for _,k in ipairs{'killed','survived','uncovered'} do sums[k]=sums[k]+(g(f.form,k) or 0) end; sums.sites=sums.sites+sites(f.form) end
    for k,v in pairs(sums) do put(c,k,v) end
  end
end
function M.apply_metrics(doc,metrics)
  if not next(metrics.crap) and not next(metrics.mutate) then return doc end
  doc=copy(doc); local classes=doc_classes(doc); local names,owned={},{}
  for ns in pairs(metrics.crap) do names[ns]=true end; for ns in pairs(metrics.mutate) do names[ns]=true end
  local sorted={}; for ns in pairs(names) do sorted[#sorted+1]=ns end; table.sort(sorted)
  for _,ns in ipairs(sorted) do local c=owner(classes,G.name(g(doc,'prefix')),ns); if c and g(c,'ns') then local key=g(c,'ns'); owned[key]=owned[key] or {}; owned[key][#owned[key]+1]=ns end end
  for _,c in ipairs(classes) do paint(c,owned[g(c,'ns')] or {},metrics) end
  return doc
end
function M.apply(doc,files) return M.apply_metrics(doc,M.load(files)) end
return M
