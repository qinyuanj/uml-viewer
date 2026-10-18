-- Policy consumes and returns uml.edn values. apply_policy(policy, graph) is the
-- IR seam; helpers below are also shared with namespace/proposal views.
-- No input is mutated. Absent optional values return Lua nil, not EDN nil.
local E=require('uml.edn')
local M={}
local K,V=E.keyword,E.vector
local function get(m,k) if E.kind(m)=='map' then local v=E.get(m,K(k)); if v~=E.nil_value then return v end end end
local function default(v,d) if v==nil or v==false or v==E.nil_value then return d end return v end
local function seq(v) return E.kind(v)=='vector' and v or {} end
local function map(t) local m=E.map(); for k,v in pairs(t) do E.assoc(m,K(k),v) end; return m end
local function copy(m) local r=E.map(); for _,e in ipairs(m) do r[#r+1]={e[1],e[2]} end; return r end
local function put(m,k,v) E.assoc(m,K(k),v); return m end
local function drop(m,k) local r=E.map(); for _,e in ipairs(m) do if not E.equal(e[1],K(k)) then r[#r+1]={e[1],e[2]} end end; return r end
local function name(x) if type(x)=='string' then return x end; assert(x and x.name,'expected namespace id'); return x.name:match('[^/]+$') end
local function id(x) return K(name(x)) end
local function ids(xs) local r=V(); for _,x in ipairs(seq(xs)) do r[#r+1]=id(x) end; return r end
local function contains(xs,x) for _,v in ipairs(xs) do if E.equal(v,x) then return true end end; return false end
local function matches(x,p) x,p=name(x),name(p); return x==p or x:sub(1,#p+1)==p..'.' end
function M.omitted_id(x,omit) for _,p in ipairs(seq(omit)) do if matches(x,p) then return true end end; return false end
function M.hierarchical(p) return get(p,'hierarchical')==true or (get(p,'packages')==nil and get(p,'diagrams')==nil) end
function M.assigned(p) local r=V(); for _,pkg in ipairs(seq(get(p,'packages'))) do for _,x in ipairs(ids(get(pkg,'nses'))) do if not contains(r,x) then r[#r+1]=x end end end; return r end
function M.unassigned(p,g)
  local r=V(); if M.hierarchical(p) then return r end; local assigned=M.assigned(p)
  for _,c in ipairs(seq(get(g,'classes'))) do if not get(c,'foreign') and not contains(assigned,get(c,'id')) then r[#r+1]=c end end
  table.sort(r,function(a,b) return name(get(a,'id'))<name(get(b,'id')) end); return r
end
function M.kind_rank(k) return ({implements=4,inheritance=4,composition=3,aggregation=2,association=1,dependency=0})[k and name(k)] or 0 end
function M.merge_edges(edges)
  local groups={}; local out=V(); local pairs={}
  for _,e in ipairs(seq(edges)) do
    local pair=V{get(e,'from'),get(e,'to')}
    local key=E.encode(pair); local group=groups[key]
    if not group then group={index=#out+1,violating=false}; groups[key]=group; out[group.index]=e; pairs[group.index]=pair end
    group.violating=group.violating or get(e,'violating')==true
    if not group.best or M.kind_rank(get(e,'kind'))>=M.kind_rank(get(group.best,'kind')) then group.best=e end
    local best=drop(group.best,'violating')
    if E.equal(get(best,'kind'),K('dependency')) and group.violating then put(best,'violating',true) end
    out[group.index]=best
  end
  local ordered=V()
  for _,i in ipairs(require('uml.domain.pair_order').indices(pairs)) do ordered[#ordered+1]=out[i] end
  return ordered
end
function M.collapse_graph(p,g)
  local remap={}; local classes=V(); local foreigns={}; local present={}
  for _,c in ipairs(seq(get(g,'classes'))) do
    local cid=get(c,'id')
    if get(c,'foreign') then
      local best
      for _,prefix in ipairs(ids(get(p,'foreign'))) do if matches(cid,prefix) and (not best or #name(prefix)>#name(best)) then best=prefix end end
      if best then remap[E.encode(cid)]=best; foreigns[name(best)]=best end
    else classes[#classes+1]=c end
  end
  local sorted={}; for n in pairs(foreigns) do sorted[#sorted+1]=n end; table.sort(sorted)
  for _,n in ipairs(sorted) do classes[#classes+1]=map{id=foreigns[n],name=n,foreign=true} end
  for _,c in ipairs(classes) do present[E.encode(get(c,'id'))]=true end
  local edges=V()
  for _,e in ipairs(seq(get(g,'edges'))) do
    local from=remap[E.encode(get(e,'from'))] or get(e,'from'); local to=remap[E.encode(get(e,'to'))] or get(e,'to')
    if present[E.encode(from)] and present[E.encode(to)] and not E.equal(from,to) then edges[#edges+1]=put(put(copy(e),'from',from),'to',to) end
  end
  return map{classes=classes,edges=M.merge_edges(edges)}
end
M.proposal_notice='PROPOSAL — not instantiated in code'
-- Optional epoch makes default-name generation deterministic for headless callers.
function M.timestamp_name(epoch) return os.date('%Y-%m-%d %H:%M:%S',epoch) end
local function nse_entry(x)
  if E.kind(x)~='map' then return id(x) end
  local cid=id(default(get(x,'id'),get(x,'label'))); local nses=V()
  for _,n in ipairs(seq(get(x,'nses'))) do nses[#nses+1]=nse_entry(n) end
  local out=copy(x); put(out,'id',cid); put(out,'label',default(get(x,'label'),name(cid))); put(out,'nses',nses)
  return out
end
function M.nse_ids(nses)
  local r=V(); for _,x in ipairs(seq(nses)) do
    if E.kind(x)=='map' then for _,n in ipairs(M.nse_ids(get(x,'nses'))) do r[#r+1]=n end else r[#r+1]=id(x) end
  end; return r
end
local function layers_from(xs)
  local r=V(); for i,x in ipairs(seq(xs)) do if E.kind(x)=='map' then
    local cid=id(default(get(x,'id'),default(get(x,'label'),'layer-'..(i-1)))); local nses=V()
    for _,n in ipairs(seq(get(x,'nses'))) do nses[#nses+1]=nse_entry(n) end
    local out=copy(x); put(out,'id',cid); put(out,'label',default(get(x,'label'),name(cid))); put(out,'nses',nses)
    r[#r+1]=out
  end end; return r
end
function M.normalize_proposal(p)
  local kind=E.kind(p); if kind~='map' and kind~='vector' then return nil end
  local layers=layers_from(kind=='vector' and p or get(p,'layers')); if #layers==0 then return nil end
  local r=map{notice=default(get(p,'notice'),M.proposal_notice),layers=layers}
  if kind=='map' then put(r,'omit',ids(get(p,'omit'))) end; return r
end
local function named(p,i)
  if E.kind(p)~='map' then return nil end
  local cid=id(default(get(p,'id'),'proposal-'..i)); local normalized=M.normalize_proposal(p)
  local out=copy(p)
  put(out,'id',cid); put(out,'name',tostring(default(get(p,'name'),default(get(p,'label'),name(cid)))))
  put(out,'layers',normalized and get(normalized,'layers') or V())
  put(out,'omit',ids(get(p,'omit'))); put(out,'notice',default(get(p,'notice'),M.proposal_notice))
  return out
end
function M.named_proposals(x)
  local xs,one=get(x,'proposals'),get(x,'proposal'); local r=V()
  if E.kind(xs)=='vector' then for i,p in ipairs(xs) do local n=named(p,i-1); if n then r[#r+1]=n end end
  elseif E.kind(one)=='map' and (get(one,'name') or #seq(get(one,'layers'))>0) then r[1]=named(one,0)
  elseif E.kind(one)=='vector' then local n=M.normalize_proposal(one); if n then r[1]=map{id=K('proposal'),name='Proposal',layers=get(n,'layers'),notice=get(n,'notice')} end end
  return r
end
function M.proposal_by_id(x,want) if want==nil or want==E.nil_value then return nil end; for _,p in ipairs(M.named_proposals(x)) do if E.equal(get(p,'id'),id(want)) then return p end end end
function M.proposal_layers(x)
  local first=M.named_proposals(x)[1]; if first then return get(first,'layers') end
  local has=false
  if E.kind(x)=='map' then local ignored; ignored,has=E.get(x,K('proposal')) end
  local value=x; if has then value=get(x,'proposal') end
  local normalized=M.normalize_proposal(value)
  return normalized and get(normalized,'layers') or V()
end
function M.level_groups(p)
  local r=V(); local levels=seq(get(p,'levels'))
  if #levels>0 then for _,group in ipairs(levels) do r[#r+1]=ids(E.kind(group)=='map' and get(group,'nses') or group) end
  else for _,layer in ipairs(M.proposal_layers(p)) do r[#r+1]=M.nse_ids(get(layer,'nses')) end end
  return r
end
function M.level_ranks(p) local r=E.map(); for i,group in ipairs(M.level_groups(p)) do for _,x in ipairs(group) do E.assoc(r,x,i-1) end end; return r end
function M.ranks_from_layers(layers)
  local levels=V(); for _,layer in ipairs(seq(layers)) do levels[#levels+1]=E.kind(layer)=='map' and M.nse_ids(get(layer,'nses')) or layer end
  return M.level_ranks(map{levels=levels})
end
function M.rank_of(x,ranks)
  local n=name(x); while n do local rank,found=E.get(ranks,K(n)); if found and rank~=E.nil_value and rank~=false then return rank end; n=n:match('^(.*)%.[^.]+$') end
end
function M.violating_dependency(e,ranks)
  if not E.equal(get(e,'kind'),K('dependency')) then return false end
  local f,t=M.rank_of(get(e,'from'),ranks),M.rank_of(get(e,'to'),ranks); return f~=nil and t~=nil and f<t
end
function M.mark_violations(edges,ranks)
  local r=V(); for _,e in ipairs(seq(edges)) do local c=drop(e,'violating'); if M.violating_dependency(e,ranks) then put(c,'violating',true) end; r[#r+1]=c end; return r
end
local function with_levels(classes,ranks,clear)
  local r=V(); for _,c in ipairs(seq(classes)) do local out=clear and drop(c,'level') or copy(c)
    local rank=not get(c,'foreign') and M.rank_of(get(c,'id'),ranks); if rank~=nil and rank~=false then put(out,'level',rank) end; r[#r+1]=out
  end; return r
end
function M.restamp_ranks(classes,edges,ranks) return map{classes=with_levels(classes,ranks,true),edges=M.mark_violations(edges,ranks)} end
function M.apply_edge_kinds(edges,kinds,omit)
  local r=V(); kinds=default(kinds,E.map())
  for _,e in ipairs(seq(edges)) do
    local pair=V{get(e,'from'),get(e,'to')}; local omitted=false
    for _,p in ipairs(seq(omit)) do if E.equal(ids(p),pair) then omitted=true end end
    if not omitted then local c=copy(e); local k=E.get(kinds,pair); if k~=nil and k~=false and k~=E.nil_value then put(c,'kind',k) end
      if not E.equal(get(c,'kind'),K('dependency')) then c=drop(c,'violating') end; r[#r+1]=c
    end
  end; return r
end
local function ir_class(c,hide)
  local r=map{id=get(c,'id'),name=default(get(c,'name'),E.nil_value)}
  for _,key in ipairs{'ns','lang','file','stereotype'} do local v=get(c,key); if v~=nil and v~=false then put(r,key,v) end end
  if get(c,'foreign') then put(r,'shape',K('oval')) elseif hide then put(r,'hide-members',true) end; return r
end
local function packaged_document(p,g)
  local index={}; for _,c in ipairs(seq(get(g,'classes'))) do index[E.encode(get(c,'id'))]=c end
  local function lookup(cid) return index[E.encode(id(cid))] end
  local function visible(nses,hide) local r=V(); for _,cid in ipairs(nses) do local c=lookup(cid); if c then r[#r+1]=ir_class(c,hide) end end; return r end
  local diagrams=V()
  for _,d in ipairs(seq(get(p,'diagrams'))) do
    local overview=E.equal(get(d,'view'),K('overview')); local packages=V(); local home=V(); local all=V(); local hide=get(d,'hide-members')
    if overview then
      for _,pkg in ipairs(seq(get(p,'packages'))) do packages[#packages+1]=map{id=id(get(pkg,'id')),label=default(get(pkg,'label'),E.nil_value),classes=visible(ids(get(pkg,'nses')),hide)} end
      local extra=M.unassigned(p,g); if #extra>0 then local cs=V(); for _,c in ipairs(extra) do cs[#cs+1]=ir_class(c,hide) end; packages[#packages+1]=map{id=K('unassigned'),label='Unassigned',classes=cs} end
      for _,pkg in ipairs(packages) do for _,c in ipairs(get(pkg,'classes')) do home[#home+1]=get(c,'id') end end
      for _,cid in ipairs(home) do all[#all+1]=cid end
    else
      local pkg; for _,candidate in ipairs(seq(get(p,'packages'))) do if E.equal(id(get(candidate,'id')),id(get(d,'package'))) then pkg=candidate; break end end
      assert(pkg,'diagram package not in policy: '..name(get(d,'package')))
      home=ids(get(pkg,'nses')); local cs=visible(home,hide); local stubs=V()
      for _,e in ipairs(seq(get(g,'edges'))) do local target=get(e,'to'); if contains(home,get(e,'from')) and not contains(home,target) and not contains(stubs,target) then stubs[#stubs+1]=target end end
      for _,cid in ipairs(stubs) do local c=lookup(cid); if c and not get(c,'foreign') then cs[#cs+1]=ir_class(c,true) end end
      packages[1]=map{id=id(get(pkg,'id')),label=default(get(pkg,'label'),E.nil_value),classes=cs}; for _,c in ipairs(cs) do all[#all+1]=get(c,'id') end
    end
    local foreign=V(); local wanted=V()
    for _,e in ipairs(seq(get(g,'edges'))) do local target=get(e,'to'); local c=lookup(target); if contains(home,get(e,'from')) and c and get(c,'foreign') and not contains(wanted,target) then wanted[#wanted+1]=target end end
    table.sort(wanted,function(a,b) return name(a)<name(b) end)
    for _,cid in ipairs(wanted) do foreign[#foreign+1]=ir_class(lookup(cid),false); all[#all+1]=cid end
    local edges=V(); for _,e in ipairs(seq(get(g,'edges'))) do if contains(all,get(e,'from')) and contains(all,get(e,'to')) and (overview or contains(home,get(e,'from')) or contains(home,get(e,'to')) or E.equal(get(e,'kind'),K('implements'))) then edges[#edges+1]=e end end
    local diagram=map{title=default(get(d,'title'),E.nil_value),direction=id(default(get(d,'direction'),overview and 'tb' or 'lr')),packages=packages,edges=M.apply_edge_kinds(M.merge_edges(edges),get(d,'edge-kinds'),get(d,'omit-edges'))}
    if #foreign>0 then put(diagram,'foreign',foreign) end; diagrams[#diagrams+1]=diagram
  end
  return map{title=default(get(p,'title'),'UML'),diagrams=diagrams}
end
function M.apply_policy(p,g)
  g=M.collapse_graph(p,g)
  local proposals=M.named_proposals(p); local proposal=M.normalize_proposal(proposals[1]) or M.normalize_proposal(get(p,'proposal')); local ranks=M.level_ranks(p)
  g=put(put(g,'classes',with_levels(get(g,'classes'),ranks,false)),'edges',M.mark_violations(get(g,'edges'),ranks))
  if not M.hierarchical(p) then return packaged_document(p,g) end
  local doc=map{title=default(get(p,'title'),'UML'),hierarchical=true,prefix=default(get(p,'prefix'),'uml-viewer'),order=ids(get(p,'order')),levels=M.level_groups(p),['edge-kinds']=default(get(p,'edge-kinds'),E.map()),['omit-edges']=default(get(p,'omit-edges'),V()),omit=ids(get(p,'omit')),classes=get(g,'classes'),edges=get(g,'edges')}
  if #proposals>0 then put(doc,'proposals',proposals) end; if proposal then put(doc,'proposal',proposal) end; return doc
end
return M
