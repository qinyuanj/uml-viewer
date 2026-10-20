-- Namespace/proposal views consume and return tagged EDN; inputs stay unchanged.
local E=require('uml.edn')
local P=require('uml.domain.policy')
local C=require('uml.domain.config')
local G=require('uml.graph')
local M={}
local K,V=E.keyword,E.vector
local function get(m,k) if E.kind(m)=='map' then local v=E.get(m,K(k)); if v~=E.nil_value then return v end end end
local function seq(x) return E.kind(x)=='vector' and x or {} end
local function map(t) return E.from_table(t) end
local function copy(m) local r=E.map(); for _,e in ipairs(m or {}) do r[#r+1]={e[1],e[2]} end; return r end
local function put(m,k,v) if v~=nil then E.assoc(m,K(k),v) end; return m end
local function drop(m,keys) local r=E.map(); for _,e in ipairs(m or {}) do if not keys[e[1].name] then r[#r+1]={e[1],e[2]} end end; return r end
local function name(x) if type(x)=='string' then return x end; return x.name:match('[^/]+$') end
local function equal(a,b) return E.equal(a,b) end
local function id(x) return K(name(x)) end
local function contains(xs,x) for _,v in ipairs(xs) do if equal(v,x) then return true end end; return false end
local function append(xs,x) xs[#xs+1]=x; return xs end
local function extend(xs,ys) for _,x in ipairs(ys) do append(xs,x) end; return xs end
local function parts(x) local r={}; for s in name(x):gmatch('[^.]+') do append(r,s) end; return r end
local function path_name(path) local r={}; for _,x in ipairs(path) do append(r,name(x)) end; return table.concat(r,'.') end
local function under(x,parent) x,parent=name(x),name(parent); return x==parent or x:sub(1,#parent+1)==parent..'.' end
local function child(x,path)
  local ss=parts(x); if #ss<#path then return nil end
  for i,p in ipairs(path) do if ss[i]~=name(p) then return nil end end
  if #ss==#path then return x end
  return K(table.concat(ss,'.',1,#path+1))
end
local function label(x)
  local last=name(x):match('[^.]+$'); local r={}
  for s in last:gmatch('[^-]+') do append(r,s:sub(1,1):upper()..(s:find('[A-Z]') and s:sub(2) or s:sub(2):lower())) end
  return table.concat(r)
end
local function module_name(c) if get(c,'proposal-group?') then return get(c,'name') or label(get(c,'id')) end; return label(get(c,'id')) end
local function box_name(leaf,cid,directory)
  local file=get(leaf,'file'); local base=file and G.file_base(file)
  if not directory and base and base:lower():sub(-3)=='.rs' then return base end
  return label(cid)
end
local function index(classes) local r={}; for _,c in ipairs(classes) do r[name(get(c,'id'))]=c end; return r end
local function descendants(cid,classes) for _,c in ipairs(classes) do local x=get(c,'id'); if not equal(x,cid) and under(x,cid) then return true end end; return false end
function M.nodes_at(classes,path)
  local r=V(); for _,c in ipairs(classes) do if not get(c,'foreign') then local x=child(get(c,'id'),path); if x and not contains(r,x) then append(r,x) end end end; return r
end
local function contents(classes,path,cid,idx)
  local next_path=V(); extend(next_path,path); append(next_path,K(name(cid):match('[^.]+$')))
  local kids=M.nodes_at(classes,next_path); local others=false
  for _,x in ipairs(kids) do if not equal(x,cid) then others=true end end
  if not others then return nil end
  local r=V(); for _,x in ipairs(kids) do local drill=not equal(x,cid) and descendants(x,classes); append(r,map{id=x,name=box_name(idx[name(x)],x,drill),['drill?']=drill}) end; return r
end
local function metrics(out,classes,rolled)
  local crap=C.pool_crap(classes,rolled); local mut=C.pool_mutants(classes); put(out,'crap',crap)
  for _,e in ipairs(mut or {}) do E.assoc(out,e[1],e[2]) end
  local lv; for _,c in ipairs(classes) do local n=get(c,'level'); if n~=nil then lv=lv and math.max(lv,n) or n end end; put(out,'level',lv); return out
end
local function view_class(classes,path,cid,idx)
  local leaf=idx[name(cid)]; local kids=contents(classes,path,cid,idx); local drill=kids~=nil
  local r=map{id=cid,name=box_name(leaf,cid,drill),['drill?']=drill}
  local subtree=V(); local lv
  for _,c in ipairs(classes) do if under(get(c,'id'),cid) then append(subtree,c); local n=get(c,'level'); if n~=nil then lv=lv and math.max(lv,n) or n end end end
  for _,k in ipairs{'ns','lang','file','stereotype','coverage','ops','fields'} do put(r,k,get(leaf,k)) end
  put(r,'level',lv)
  if drill then metrics(r,subtree); put(r,'contents',kids); put(r,'hide-members',true)
  else
    local crap=get(leaf,'crap'); if get(crap,'mu')~=nil then put(r,'crap',crap) end
    if get(leaf,'killed')~=nil or get(leaf,'survived')~=nil then for _,k in ipairs{'killed','survived','uncovered'} do put(r,k,get(leaf,k)) end end
  end
  return r
end
local function external(cid,path,idx)
  local c=idx[name(cid)]
  if get(c,'foreign') then return map{id=cid,name=get(c,'name') or name(cid),foreign=true} end
  if child(cid,path) then return nil end
  local ss=parts(cid); local x=K(table.concat(ss,'.',1,math.min(#ss,math.max(1,#path))))
  return map{id=x,name=label(x),external=true}
end
local function collapse(cid,path,idx) if get(idx[name(cid)],'foreign') then return cid end; return child(cid,path) end
local function port(ports,cid,dep)
  local key=name(cid); ports[key]=ports[key] or V()
  for _,p in ipairs(ports[key]) do if equal(get(p,'id'),get(dep,'id')) then return end end
  append(ports[key],map{id=get(dep,'id'),name=get(dep,'name')})
end
function M.view_at(doc,path)
  path=path or V(); local classes=V()
  for _,c in ipairs(seq(get(doc,'classes'))) do if not P.omitted_id(get(c,'id'),get(doc,'omit')) then append(classes,c) end end
  local idx=index(classes); local nodes=M.nodes_at(classes,path); local order=get(doc,'order'); local specific=E.kind(order)=='map' and E.get(order,path_name(path))
  order=specific or (#path==0 and order) or V(); local ranks={}; for i,x in ipairs(seq(order)) do ranks[name(x)]=i end
  table.sort(nodes,function(a,b) local ar,br=ranks[name(a)] or math.huge,ranks[name(b)] or math.huge; if ar~=br then return ar<br end; return name(a)<name(b) end)
  local boxes=V(); for _,cid in ipairs(nodes) do append(boxes,view_class(classes,path,cid,idx)) end
  local internal,foreign,foreign_edges=V(),V(),V(); local incoming,outgoing={},{}
  local function add_foreign(box,dep,e)
    local fid=get(dep,'id'); local out=copy(e); put(out,'from',box); put(out,'to',fid); append(foreign_edges,out)
    for _,c in ipairs(foreign) do if equal(get(c,'id'),fid) then return end end
    append(foreign,map{id=fid,name=get(dep,'name'),foreign=true,shape=K('oval')})
  end
  for _,e in ipairs(seq(get(doc,'edges'))) do
    local f,t=collapse(get(e,'from'),path,idx),collapse(get(e,'to'),path,idx)
    local fh,th=f and contains(nodes,f),t and contains(nodes,t)
    local te=not th and external(get(e,'to'),path,idx); local fe=not fh and external(get(e,'from'),path,idx)
    if fh and th and not equal(f,t) then append(internal,put(put(copy(e),'from',f),'to',t))
    elseif fh and get(te,'foreign') then add_foreign(f,te,e)
    elseif fh and te then port(outgoing,f,te)
    elseif th and get(fe,'foreign') then add_foreign(t,fe,put(put(copy(e),'from',get(fe,'id')),'to',t))
    elseif th and fe then port(incoming,t,fe) end
  end
  for _,c in ipairs(boxes) do local n=name(get(c,'id')); put(c,'in-deps',incoming[n]); put(c,'out-deps',outgoing[n]) end
  local edges=P.apply_edge_kinds(P.merge_edges(extend(internal,foreign_edges)),get(doc,'edge-kinds'),get(doc,'omit-edges'))
  local visible=V(); extend(visible,nodes); for _,c in ipairs(foreign) do append(visible,get(c,'id')) end
  local filtered=V(); for _,e in ipairs(edges) do if contains(visible,get(e,'from')) and contains(visible,get(e,'to')) then append(filtered,e) end end
  local title=#path>0 and path_name(path) or get(doc,'title') or 'UML'
  local r=map{title=title,direction=K('tb'),packages=V{map{id=K('view'),label=title,classes=boxes}},edges=filtered}
  if #foreign>0 then put(r,'foreign',foreign) end; return r
end
M.proposal_layers=P.proposal_layers
M.named_proposals=P.named_proposals
local function chosen(doc,which)
  local named=E.kind(which)=='map' and get(which,'layers') and which or which and P.proposal_by_id(doc,which) or P.named_proposals(doc)[1]
  return P.normalize_proposal(named) or P.normalize_proposal(get(doc,'proposal')),named
end
local function covering(cid,visible,owners)
  if contains(visible,cid) then return cid end
  local owner=owners[name(cid)]; if owner and contains(visible,owner) then return owner end
  local s=name(cid):match('^(.*)%.[^.]+$')
  while s do if contains(visible,K(s)) then return K(s) end; s=s:match('^(.*)%.[^.]+$') end
end
local function remap_edges(doc,classes,foreign,owners,ranks)
  local visible=V(); for _,c in ipairs(classes) do append(visible,get(c,'id')) end; for _,c in ipairs(seq(foreign)) do append(visible,get(c,'id')) end
  local edges=V()
  for _,e in ipairs(seq(get(doc,'edges'))) do local f,t=covering(get(e,'from'),visible,owners),covering(get(e,'to'),visible,owners)
    if f and t and not equal(f,t) then append(edges,put(put(copy(e),'from',f),'to',t)) end
  end
  return get(P.restamp_ranks(classes,P.apply_edge_kinds(P.merge_edges(edges),get(doc,'edge-kinds'),get(doc,'omit-edges')),ranks),'edges')
end
local function group_box(group,kids)
  local cs=V(); for _,c in ipairs(kids) do append(cs,map{id=get(c,'id'),name=module_name(c),['drill?']=get(c,'drill?')==true}) end
  return metrics(map{id=get(group,'id'),name=get(group,'label') or label(get(group,'id')),['drill?']=true,['proposal-group?']=true,nses=get(group,'nses'),members=kids,['hide-members']=true,contents=cs},kids)
end
function M.proposal_view(doc,which)
  local root=M.view_at(doc,V()); local proposal,named=chosen(doc,which); local layers=seq(get(proposal,'layers')); local ranks=P.ranks_from_layers(layers)
  local omit=V(); extend(omit,seq(get(doc,'omit'))); extend(omit,seq(get(proposal,'omit')))
  local leaves=V(); for _,c in ipairs(seq(get(doc,'classes'))) do if not get(c,'foreign') and not P.omitted_id(get(c,'id'),omit) then append(leaves,c) end end
  local stamped=get(P.restamp_ranks(leaves,V(),ranks),'classes'); local idx=index(stamped); local claimed=V(); local owners={}
  local function register(nses)
    for _,x in ipairs(seq(nses)) do if E.kind(x)=='map' then
      for _,cid in ipairs(P.nse_ids(get(x,'nses'))) do owners[name(cid)]=get(x,'id') end
      register(get(x,'nses'))
    end end
  end
  local function as_module(c) return put(copy(c),'name',module_name(c)) end
  local function pick(x)
    local r=V()
    if E.kind(x)=='map' then local kids=V(); for _,n in ipairs(seq(get(x,'nses'))) do extend(kids,pick(n)) end; if #kids>0 then append(r,group_box(x,kids)) end
    elseif idx[name(x)] then append(r,as_module(idx[name(x)]))
    else for _,c in ipairs(stamped) do if under(get(c,'id'),x) then append(r,as_module(c)) end end end
    return r
  end
  local pkgs=V()
  for _,layer in ipairs(layers) do
    extend(claimed,P.nse_ids(get(layer,'nses'))); register(get(layer,'nses'))
    local cs=V(); for _,x in ipairs(seq(get(layer,'nses'))) do extend(cs,pick(x)) end
    if #cs>0 then append(pkgs,map{id=K('proposal.'..name(get(layer,'id'))),label=get(layer,'label'),classes=cs}) end
  end
  local extras=V(); for _,c in ipairs(stamped) do local covered=false
    for _,cid in ipairs(claimed) do if under(get(c,'id'),cid) or under(cid,get(c,'id')) then covered=true end end
    if not covered then append(extras,as_module(c)) end
  end
  if #extras>0 then append(pkgs,map{id=K('proposal.unassigned'),label='Unassigned',classes=extras}) end
  local reversed=V(); for i=#pkgs,1,-1 do append(reversed,pkgs[i]) end; pkgs=reversed
  local all=V(); for _,p in ipairs(pkgs) do extend(all,get(p,'classes')) end
  if #pkgs>0 then put(root,'title',get(named,'notice') or get(proposal,'notice') or P.proposal_notice); put(root,'proposal',true); put(root,'packages',pkgs); put(root,'edges',remap_edges(doc,all,get(root,'foreign'),owners,ranks)) end
  return root
end
function M.find_nested_group(doc,which,cid)
  if cid==nil then return nil end; cid=id(cid); local proposal=chosen(doc,which)
  local function find(nses) for _,x in ipairs(seq(nses)) do if E.kind(x)=='map' then if equal(get(x,'id'),cid) then return x end; local found=find(get(x,'nses')); if found then return found end end end end
  for _,layer in ipairs(seq(get(proposal,'layers'))) do local found=find(get(layer,'nses')); if found then return found end end
end
function M.layer_view(doc,which,pkg_id)
  local root=M.proposal_view(doc,which)
  for _,p in ipairs(get(root,'packages')) do if equal(get(p,'id'),pkg_id) then put(root,'title',get(p,'label') or name(pkg_id)); put(root,'packages',V{p}); return root end end
  for _,p in ipairs(get(root,'packages')) do for _,c in ipairs(get(p,'classes')) do if equal(get(c,'id'),pkg_id) and get(c,'proposal-group?') and #seq(get(c,'members'))>0 then
    local cs=get(c,'members'); local title=get(c,'name') or name(pkg_id); local proposal=chosen(doc,which)
    put(root,'title',title); put(root,'packages',V{map{id=pkg_id,label=title,classes=cs}})
    put(root,'edges',remap_edges(doc,cs,get(root,'foreign'),{},P.ranks_from_layers(seq(get(proposal,'layers'))))); return root
  end end end
  return root
end
function M.proposal_package_id(cid) return cid~=nil and name(cid):sub(1,9)=='proposal.' end
M.declutter_modes=E.parse('[:full :arrows :triangles :elements :classes]')
function M.next_declutter(mode)
  local i=1; for j,x in ipairs(M.declutter_modes) do if equal(x,mode) then i=j end end
  return M.declutter_modes[i%#M.declutter_modes+1]
end
local function dummy(p) return map{id=get(p,'id'),name=get(p,'label') or name(get(p,'id')),['dummy?']=true,['drill?']=true,['hide-members']=true} end
local function packages(view,transform)
  local r=copy(view); local ps=V(); for _,p in ipairs(seq(get(view,'packages'))) do append(ps,transform(copy(p))) end; put(r,'packages',ps); return r
end
function M.collapse_arrows(view)
  local multi=#seq(get(view,'packages'))>1
  view=packages(view,function(p)
    local cs=get(p,'classes'); local found=false
    for _,c in ipairs(cs) do if equal(get(c,'id'),get(p,'id')) then found=true end end
    if multi and not found then local next_cs=V{metrics(dummy(p),cs)}; extend(next_cs,cs); put(p,'classes',next_cs) end; return p
  end)
  local owners,known={},{}
  for _,p in ipairs(get(view,'packages')) do for _,c in ipairs(get(p,'classes')) do local n=name(get(c,'id')); owners[n]=get(p,'id'); known[n]=true end end
  for _,c in ipairs(seq(get(view,'foreign'))) do known[name(get(c,'id'))]=true end
  local groups,order={},{}
  for _,e in ipairs(seq(get(view,'edges'))) do
    local of,ot=get(e,'from'),get(e,'to'); local f=multi and owners[name(of)] or of; local t=multi and owners[name(ot)] or ot; f,t=f or of,t or ot
    if known[name(f)] and known[name(t)] and not equal(f,t) then
      local key=E.encode(V{f,t}); local g=groups[key]
      if not g then g={edges=V(),f=f,t=t}; groups[key]=g; append(order,g) end
      append(g.edges,put(put(put(put(copy(e),'from',f),'to',t),'orig-from',of),'orig-to',ot))
    end
  end
  local edges=V()
  for _,g in ipairs(order) do
    local best,viol; local via,deps=V(),V()
    for _,e in ipairs(g.edges) do
      if get(e,'violating') and not viol then best=e; viol=true
      elseif not viol and (not best or P.kind_rank(get(e,'kind'))>=P.kind_rank(get(best,'kind'))) then best=e end
      for _,k in ipairs{'from','to','orig-from','orig-to'} do local x=get(e,k); if x and not contains(via,x) then append(via,x) end end
      append(deps,map{from=get(e,'orig-from') or get(e,'from'),to=get(e,'orig-to') or get(e,'to'),violating=get(e,'violating')==true})
    end
    local out=drop(best,{violating=true,['orig-from']=true,['orig-to']=true}); put(out,'via-ids',via); put(out,'deps',deps)
    if viol then put(out,'kind',K('dependency')); put(out,'violating',true) end; append(edges,out)
  end
  return put(view,'edges',edges)
end
function M.hide_elements(view)
  return packages(view,function(p) local cs=V(); for _,c in ipairs(get(p,'classes')) do append(cs,put(drop(c,{contents=true,['in-deps']=true,['out-deps']=true}),'hide-members',true)) end; return put(p,'classes',cs) end)
end
function M.hide_classes(view)
  local multi=#seq(get(view,'packages'))>1
  return packages(view,function(p)
    local cs=V(); local real=V(); local endpoint
    for _,c in ipairs(get(p,'classes')) do if get(c,'dummy?') then endpoint=endpoint or c else append(real,c) end end
    if multi then
      endpoint=drop(endpoint or dummy(p),{crap=true,killed=true,survived=true,uncovered=true,['mut-gap']=true})
      cs=V{metrics(endpoint,real)}; local nses=V(); for _,c in ipairs(real) do append(nses,get(c,'id')) end; put(p,'nses',nses)
    else for _,c in ipairs(real) do append(cs,put(drop(c,{contents=true}),'hide-members',true)) end end
    return put(p,'classes',cs)
  end)
end
function M.apply_declutter(view,mode)
  mode=mode and name(mode) or 'full'; if mode=='methods' then mode='elements' end
  if mode=='arrows' or mode=='triangles' or mode=='elements' or mode=='classes' then view=M.collapse_arrows(view) end
  if mode=='triangles' then view=put(copy(view),'hide-edges',true) end
  if mode=='elements' or mode=='classes' then view=M.hide_elements(view) end
  if mode=='classes' then view=M.hide_classes(view) end
  return view
end
return M
