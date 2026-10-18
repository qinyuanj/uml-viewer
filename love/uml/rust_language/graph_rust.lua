-- Rust graph parity: canonical file snapshots are supplied by the IO adapter.
local E=require('uml.edn')
local G=require('uml.graph')
local M={}
local K,V=E.keyword,E.vector
local function record(t) local r=E.map(); for k,v in pairs(t) do E.assoc(r,K(k),v) end; return r end
local function get(m,k,d) local v=E.get(m,K(k)); if v==nil or v==E.nil_value or v==false then return d end; return v end
local function unique(xs,x) for _,v in ipairs(xs) do if E.equal(v,x) then return end end; xs[#xs+1]=x end
local function trim(s) return (s:gsub('^%s+',''):gsub('%s+$','')) end
local function in_hole(holes,i) for _,h in ipairs(holes) do if i>=h[1] and i<h[2] then return true end end; return false end
local function mask(source)
  local out,holes={},{}; local i=1
  while i<=#source do
    local c,n=source:sub(i,i),source:sub(i+1,i+1); local finish
    if G.line_comment(c,n) then i=G.mask_line_comment(source,out,i)
    elseif G.block_comment(c,n) then i=G.mask_block_comment(source,out,i)
    else
      if c=='"' then finish=G.scan_quoted(source,i,c)
      elseif c=="'" and n~='' and n~='\\' and source:sub(i+2,i+2)=="'" then finish=i+3
      elseif c=="'" and n=='\\' then finish=(source:find("'",i+3,true) or #source)+1 end
      if finish then holes[#holes+1]={i,finish}; out[#out+1]=source:sub(i,finish-1); i=finish
      else out[#out+1]=c; i=i+1 end
    end
  end
  return table.concat(out),holes
end
local function paths(path)
  path=path:gsub('{[^\n]*',''):gsub(' as [^\n]*$',''); local xs=V()
  for s in (path..'::'):gmatch('(.-)::') do s=trim(s); if s~='' then xs[#xs+1]=s end end
  return xs
end
local function command(text,at)
  local prefix=text:sub(math.max(1,at-240),at-1)
  for p in prefix:gmatch('()#%[') do
    local rest=prefix:sub(p):match('^#%[%s*tauri%s*::%s*command%f[^%w_][^%]]*%](.*)$')
    if rest then rest=rest:gsub('^%s*pub%b()%s+',''):gsub('^%s*pub%s+',''):gsub('^%s*async%s+',''); if rest:match('^%s*$') then return true end end
  end
  return false
end
function M.read_module(source)
  local text,holes=mask(source); local uses,mods,impls,commands,ops=V(),V(),V(),V(),V(); local bindings=E.map(); local publics={}
  -- Java's multiline ^ includes the whitespace preceding the declaration in the match.
  local use_end,mod_end=0,0
  for i=1,#text do
    if i==1 or text:sub(i-1,i-1)=='\n' then
      local rest=text:sub(i); local body=rest:gsub('^%s*pub%b()%s+',''):gsub('^%s*pub%s+','')
      local use=body:match('^%s*use%s+([^;]+);'); local mod=body:match('^%s*mod%s+([A-Za-z_][A-Za-z0-9_]*)%s*;')
      if use and i>use_end then use_end=i+(rest:find(';',1,true) or #rest)-1; if not in_hole(holes,i) then uses[#uses+1]=trim(use) end end
      if mod and i>mod_end then mod_end=i+(rest:find(';',1,true) or #rest)-1; if not in_hole(holes,i) and mod~='tests' then mods[#mods+1]=mod end end
    end
  end
  for at in text:gmatch('()%f[%w_]pub%f[^%w_]') do
    if not in_hole(holes,at) then local rest=text:sub(at+3); rest=rest:gsub('^(%s+)async%s+','%1'); local n=rest:match('^%s+fn%s+([A-Za-z_][A-Za-z0-9_]*)'); if n then publics[n]=true end end
  end
  local functions={}
  for at in text:gmatch('()%f[%w_]fn%f[^%w_]') do
    if not in_hole(holes,at) then local n=text:sub(at):match('^fn%s+([A-Za-z_][A-Za-z0-9_]*)'); if n then functions[#functions+1]=n; if command(text,at) then unique(commands,n) end end end
  end
  for _,n in ipairs(functions) do if publics[n] then unique(ops,n) else for _,cmd in ipairs(commands) do if cmd==n then unique(ops,n) end end end end
  for at in text:gmatch('()%f[%w_]impl%f[^%w_]') do
    if not in_hole(holes,at) then
      local rest=text:sub(at+4)
      local trait=rest:match('^%s+([A-Za-z_][A-Za-z0-9_:]*)%s+for%s+')
      if rest:match('^%s*<') then
        local generic=rest:match('^%s*<([^;{]*)') or ''
        local offset=rest:find('<',1,true)
        for close in generic:gmatch('()>') do
          local candidate=rest:sub(offset+close+1):match('^%s+([A-Za-z_][A-Za-z0-9_:]*)%s+for%s+')
          if candidate then trait=candidate end
        end
      end
      if trait and not trait:find(':::',1,true) and trait:sub(-1)~=':' then impls[#impls+1]=trait end
    end
  end
  for _,use in ipairs(uses) do local segs=paths(use); if (segs[1]=='crate' or segs[1]=='self' or segs[1]=='super') and segs[2] then E.assoc(bindings,segs[#segs],segs) end end
  return record{uses=uses,mods=mods,impls=impls,bindings=bindings,commands=commands,ops=ops}
end
local function parent(path) return path:match('^(.*)/[^/]+$') end
function M.accepts(relative) return relative:match('%.rs$')~=nil or relative:match('/Cargo%.toml$')~=nil or relative=='Cargo.toml' end
-- Only this explicitly named metadata is collected outside the source tree.
function M.metadata_paths(root) local p=parent(root); return {root..'/Cargo.toml',p and p..'/Cargo.toml' or nil} end
local function lib_name(root,by_path)
  local cargo
  for _,path in ipairs(M.metadata_paths(root)) do if by_path[path] then cargo=by_path[path].source; break end end
  if not cargo then return nil end
  local a,b=cargo:find('[lib]',1,true); if not a then return nil end
  local finish=cargo:find('\n[',b+1,true); local body=cargo:sub(b+1,finish and finish-1 or #cargo)
  return body:match('^name%s*=%s*"([^"]+)"') or body:match('\nname%s*=%s*"([^"]+)"')
end
local function module_relative(path,root)
  local rel=G.relative_path(path,root)
  if rel=='lib.rs' or rel=='mod.rs' then return '' end
  return (rel:gsub('/mod%.rs$',''):gsub('%.rs$',''):gsub('/','.'))
end
local function resolve_mod(path,name,by_path)
  local base=G.file_base(path); local dir=parent(path)
  if base~='lib.rs' and base~='main.rs' and base~='mod.rs' then dir=dir..'/'..base:gsub('%.rs$','') end
  for _,p in ipairs{dir..'/'..name..'.rs',dir..'/'..name..'/mod.rs'} do if by_path[p] then return p end end
end
function M.scan(root,opts,files,cwd)
  assert(type(cwd)=='string','canonical working directory required'); opts=opts or E.map()
  local prefix=get(opts,'prefix','rust'); local ns_prefix=get(opts,'ns-prefix',prefix)
  local by_path,surfaces={},{}; for _,f in ipairs(files) do by_path[f.path]=f end
  local function surface(path) if not surfaces[path] then surfaces[path]=M.read_module(by_path[path].source) end; return surfaces[path] end
  local queue,seen,reachable={},{},{}
  for _,n in ipairs{'lib.rs','main.rs'} do local p=root..'/'..n; if by_path[p] then queue[#queue+1]=p end end
  local at=1
  while at<=#queue do local path=queue[at]; at=at+1
    if not seen[path] then seen[path]=true; reachable[#reachable+1]=path
      for _,n in ipairs(get(surface(path),'mods')) do local child=resolve_mod(path,n,by_path); if child and not seen[child] then queue[#queue+1]=child end end
    end
  end
  table.sort(reachable)
  local metas,by_rel,by_file={}, {}, {}; local lib_id; local name=lib_name(root,by_path)
  for _,path in ipairs(reachable) do
    local rel=module_relative(path,root); local ns=G.ns_join(ns_prefix,rel); local m={path=path,relative=rel,ns=ns,id=G.id_of(ns,prefix)}
    metas[#metas+1]=m; by_file[path]=m; if not by_rel[rel] then by_rel[rel]=m.id end; if rel=='' and not lib_id then lib_id=m.id end
  end
  local function suffix(s)
    s=s:gsub('^%.+',''):gsub('%.+$',''); if s=='' then return nil end
    return by_rel[s] or (s:find('.',1,true) and by_rel[s:match('^(.*)%.[^.]*$')] or nil)
  end
  local function resolve(segs,m)
    local head=segs[1]; local tail=table.concat(segs,'.',2)
    if head=='crate' then return suffix(tail)
    elseif head=='self' then return suffix(G.ns_join(m.relative,tail))
    elseif head=='super' then return suffix(G.ns_join(m.relative:match('^(.*)%.[^.]*$') or '',tail))
    elseif head==name then return suffix(tail) or lib_id end
  end
  local classes,edges,foreigns,commands=V(),V(),V(),E.map(); local project_ids={}
  for _,m in ipairs(metas) do project_ids[E.encode(m.id)]=true end
  for _,m in ipairs(metas) do
    local s=surface(m.path); local ops=V(); for _,n in ipairs(get(s,'ops')) do ops[#ops+1]=record{name=n,text=n} end
    local c=record{id=m.id,ns=m.ns,name=G.file_base(m.path),lang=K('rust'),file=G.relative_path(m.path,cwd)}
    if #ops>0 then E.assoc(c,K('ops'),ops) end; classes[#classes+1]=c
    local requires,externs,impls=V(),V(),V()
    for _,n in ipairs(get(s,'mods')) do local path=resolve_mod(m.path,n,by_path); if path then unique(requires,by_file[path].id) end end
    for _,use in ipairs(get(s,'uses')) do
      local segs=paths(use); local target=resolve(segs,m); if target then unique(requires,target) end
      local head=segs[1]; if head and head~='crate' and head~='self' and head~='super' and head~=name then unique(externs,K(head)); unique(foreigns,K(head)) end
    end
    if lib_id and name then
      local text=mask(by_path[m.path].source); local pos=1
      while true do local a,b=text:find(name..'::',pos,true); if not a then break end
        if a==1 or not text:sub(a-1,a-1):match('[%w_]') then unique(requires,lib_id); break end; pos=b+1
      end
    end
    for _,trait in ipairs(get(s,'impls')) do
      local bound=E.get(get(s,'bindings'),trait); local segs=paths(trait)
      local target=(bound and resolve(bound,m)) or resolve(segs,m) or by_rel[segs[1]]
      if target then unique(impls,target) end
    end
    for _,pair in ipairs{{requires,'dependency'},{externs,'dependency'},{impls,'implements'}} do
      for _,to in ipairs(pair[1]) do if not E.equal(m.id,to) then unique(edges,record{from=m.id,to=to,kind=K(pair[2])}) end end
    end
    if #get(s,'commands')>0 then E.assoc(commands,m.id,get(s,'commands')) end
  end
  for _,id in ipairs(foreigns) do if not project_ids[E.encode(id)] then classes[#classes+1]=G.foreign_class(id) end end
  return record{classes=classes,edges=edges,commands=commands}
end
return M
