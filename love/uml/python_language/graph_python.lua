-- Python's JVM scanner semantics, over caller-owned canonical source snapshots.
local E=require('uml.edn')
local G=require('uml.graph')
local M={}
local K,V=E.keyword,E.vector
local function record(t) return E.from_table(t) end
local function get(m,k,d) local v=E.get(m,K(k)); if v==nil or v==E.nil_value or v==false then return d end; return v end
local function trim(s) return (s:gsub('^%s+',''):gsub('%s+$','')) end
local function ident(c) return c~='' and c:match('[%w_]')~=nil end
local function in_hole(holes,i) for _,h in ipairs(holes) do if i>=h[1] and i<h[2] then return true end end; return false end
local function flags(s,i)
  local q=s:sub(i,i); if q~='"' and q~="'" then return nil end
  local j,raw,f=i-1,false,false
  while j>0 and s:sub(j,j):match('[rRuUbBfF]') do local c=s:sub(j,j):lower(); raw=raw or c=='r'; f=f or c=='f'; j=j-1 end
  if j==0 or not ident(s:sub(j,j)) then return raw,f end
end
local skip_string
skip_string=function(s,i)
  local raw,f=flags(s,i); if raw==nil then return nil end
  local q=s:sub(i,i); local len=s:sub(i,i+2)==q:rep(3) and 3 or 1
  local j,depth=i+len,0
  while j<=#s do
    local c,n=s:sub(j,j),s:sub(j+1,j+1)
    if c=='\\' and j<#s and (not raw or n==q) then j=j+2
    elseif f and ((c=='{' and n=='{') or (c=='}' and n=='}')) then j=j+2
    elseif f and c=='{' then depth=depth+1; j=j+1
    elseif f and c=='}' and depth>0 then depth=depth-1; j=j+1
    elseif depth>0 and (c=='"' or c=="'") then j=skip_string(s,j) or j+1
    elseif depth==0 and s:sub(j,j+len-1)==q:rep(len) then return j+len
    else j=j+1 end
  end
  return #s+1
end
local function mask(s)
  local out,holes={},{}; local i=1
  while i<=#s do
    if s:sub(i,i)=='#' then local j=s:find('\n',i,true) or #s+1; out[#out+1]=(' '):rep(j-i); i=j
    else local j=skip_string(s,i)
      if j then out[#out+1]=s:sub(i,j-1); holes[#holes+1]={i,j}; i=j
      else out[#out+1]=s:sub(i,i); i=i+1 end
    end
  end
  return table.concat(out),holes
end
local function statement_end(s,i)
  local p,b,c=0,0,0
  while i<=#s do
    local x=s:sub(i,i)
    if x=='\\' and s:sub(i+1,i+1)=='\n' then i=i+2
    elseif x=='\n' and p==0 and b==0 and c==0 then return i
    else
      if x=='(' then p=p+1 elseif x==')' then p=math.max(0,p-1)
      elseif x=='[' then b=b+1 elseif x==']' then b=math.max(0,b-1)
      elseif x=='{' then c=c+1 elseif x=='}' then c=math.max(0,c-1) end
      i=i+1
    end
  end
  return #s+1
end
local function parts(s)
  local out=V(); s=s:gsub('[()]',' ')
  for part in (s..','):gmatch('(.-),') do
    part=trim(part); local name,bind=part:match('^([%a_][%w_%.]*)%s+as%s+([%a_][%w_]*)$')
    name=name or part:match('^([%a_][%w_%.]*)$')
    if name then out[#out+1]=record{name=name,bind=bind or name:match('^[^.]+')} end
  end
  return out
end
local function parse_import(s)
  s=trim(s:gsub('\\\n',' '):gsub('%s+',' '))
  if s:sub(1,5)=='from ' then
    local path,names=s:match('^from%s+([^ ]*)%s+import%s+(.+)$')
    if path then
      local dots,mod=path:match('^(%.*)(.*)$')
      if mod=='' or mod:match('^[%a_][%w_%.]*$') then return record{kind=K('from'),dots=#dots,module=mod~='' and mod or E.nil_value,names=parts(names)} end
    end
  elseif s:sub(1,7)=='import ' then return record{kind=K('import'),modules=parts(s:sub(8))} end
end
function M.read_module(source)
  local text,holes=mask(source); local imports,defs=V(),V(); local i=1
  while i<=#text do
    local word=text:sub(i):match('^(%a+)')
    if (word=='from' or word=='import') and not ident(text:sub(i-1,i-1)) and not ident(text:sub(i+#word,i+#word)) and not in_hole(holes,i) and (text:sub(1,i-1):match('([^\n;]*)$') or ''):match('^%s*$') then
      local finish=statement_end(text,i); local imp=parse_import(text:sub(i,finish-1)); if imp then imports[#imports+1]=imp end; i=finish
    else i=i+1 end
  end
  i=1
  while i<=#text do
    local line=text:sub(i):match('^[^\n]*') or ''
    local name=line:match('^async[ \t]+def[ \t]+([%a_][%w_]*)') or line:match('^def[ \t]+([%a_][%w_]*)') or line:match('^class[ \t]+([%a_][%w_]*)')
    if name and name:sub(1,1)~='_' and not in_hole(holes,i) then
      local header=text:sub(i,statement_end(text,i)-1):gsub('%s+',' ')
      if line:match('^class') then
        local bases=V(); local body=header:match('%(([^)]*)%)%s*:')
        if body then for part in (body..','):gmatch('(.-),') do local base=trim(part):match('^([%a_][%w_%.]*)$'); if base then bases[#bases+1]=base end end end
        defs[#defs+1]=record{name=name,kind=K('class'),bases=body and bases or E.nil_value}
      else defs[#defs+1]=record{name=name,kind=K('function'),['return']=header:match('%)%s*%->%s*([%a_][%w_]*)%s*:') or E.nil_value} end
    end
    i=i+#line+1
  end
  return record{imports=imports,defs=defs}
end
local skip={__pycache__=true,venv=true,['site-packages']=true,dist=true,build=true,node_modules=true,tests=true,__pypackages__=true}
function M.accepts_directory(relative)
  for dir in relative:gmatch('[^/]+') do if dir:sub(1,1)=='.' or skip[dir] then return false end end
  return true
end
function M.accepts(relative)
  local name=relative:match('[^/]+$') or ''
  return name:match('%.py$')~=nil and name~='conftest.py' and not name:match('^test_.+%.py$') and not name:match('^.+_test%.py$') and not name:match('^.+_tests%.py$') and M.accepts_directory(relative:match('^(.*)/') or '')
end
local function parent(s) return s and s:match('^(.*)%.[^.]+$') end
local function join(a,b) if not b or b=='' then return a end; if not a or a=='' then return b end; return a..'.'..b end
local function marker(s) local n=s:match('[^.]+$'); return n=='Protocol' or n=='ABC' or n=='ABCMeta' end
local function unique(xs,x) for _,v in ipairs(xs) do if E.equal(v,x) then return end end; xs[#xs+1]=x end
function M.scan(root,opts,files,cwd)
  assert(type(cwd)=='string','canonical working directory required'); opts=opts or E.map()
  local prefix=get(opts,'prefix','app'); local ns_prefix=get(opts,'ns-prefix',prefix)
  local metas,by_ns,by_rel={},{},{}; local sorted={}
  for _,f in ipairs(files) do if M.accepts(G.relative_path(f.path,root),opts) then sorted[#sorted+1]=f end end
  table.sort(sorted,function(a,b) return a.path<b.path end)
  for _,f in ipairs(sorted) do
    local rel=G.relative_path(f.path,root):gsub('%.py$','')
    if rel=='__init__' then rel='' else rel=rel:gsub('/__init__$','') end; rel=rel:gsub('/','.')
    local ns
    if rel==ns_prefix or rel:sub(1,#ns_prefix+1)==ns_prefix..'.' then ns=rel else ns=G.ns_join(ns_prefix,rel) end
    local id=G.id_of(ns,prefix); local m={file=f,relative=rel,ns=ns,id=id,init=f.path:match('/__init__%.py$')~=nil}
    metas[#metas+1]=m; by_ns[ns]=id; if rel~='' then by_rel[rel]=id end
  end
  local function exact(s) if s and s~='' then return by_ns[s] or by_rel[s] end end
  local classes,edges,foreigns=V(),V(),V(); local project={}; for _,m in ipairs(metas) do project[m.id.name]=true end
  for _,m in ipairs(metas) do
    local surface=M.read_module(m.file.source); local hits={}; local package=m.init and m.ns or (parent(m.ns) or '')
    for _,imp in ipairs(get(surface,'imports')) do
      if get(imp,'kind').name=='import' then
        for _,part in ipairs(get(imp,'modules')) do
          local n=get(part,'name'); local id=exact(n)
          hits[#hits+1]=id and {id=id,bind=get(part,'bind'),path=n} or {foreign=n}
        end
      else
        local dots=get(imp,'dots'); local mod=get(imp,'module'); local rootmod=mod
        if dots>0 then
          rootmod=package; local left=dots-1
          while left>0 do if rootmod=='' then rootmod=nil; break end; rootmod=parent(rootmod) or ''; left=left-1 end
          if rootmod~=nil then rootmod=join(rootmod,mod) end
        end
        if rootmod~=nil then
          local relative_only=dots>0 and (not mod or mod==''); local rootid=exact(rootmod); local found=false
          for _,part in ipairs(get(imp,'names')) do
            local id=exact(join(rootmod,get(part,'name'))) or (not relative_only and rootid or nil)
            if id then hits[#hits+1]={id=id,bind=get(part,'bind')}; found=true end
          end
          if not found and not relative_only then
            if rootid then hits[#hits+1]={id=rootid} elseif rootmod~='' then hits[#hits+1]={foreign=rootmod} end
          end
        end
      end
    end
    local bindings,paths,requires,external={},{},V(),V()
    for _,h in ipairs(hits) do
      if h.bind then bindings[h.bind]=h.id end; if h.path then paths[h.path]=h.id end
      if h.id then unique(requires,h.id) end; if h.foreign then unique(external,K(h.foreign)); unique(foreigns,K(h.foreign)) end
    end
    local function target(expr)
      if not expr then return nil end
      local id=bindings[expr] or paths[expr]; if id then return id end
      local p=parent(expr)
      while p do id=paths[p] or bindings[p]; if id then return id end; p=parent(p) end
    end
    local inherits,impls,ops=V(),V(),V(); local class_count,interface=0,true
    for _,d in ipairs(get(surface,'defs')) do
      local n=get(d,'name'); ops[#ops+1]=record{name=n,text=n}
      if get(d,'kind').name=='function' then interface=false else
        class_count=class_count+1; local bases=get(d,'bases',V()); if #bases==0 then interface=false end
        for _,base in ipairs(bases) do if not marker(base) then interface=false end end
      end
      for _,base in ipairs(get(d,'bases',V())) do local id=target(base); if id then unique(marker(base) and impls or inherits,id) end end
    end
    for _,d in ipairs(get(surface,'defs')) do local ret=get(d,'return'); if ret and bindings[ret] then unique(impls,bindings[ret]) end end
    classes[#classes+1]=record{id=m.id,ns=m.ns,name=G.module_name(m.id),lang=K('python'),file=G.relative_path(m.file.path,cwd),ops=#ops>0 and ops or nil,stereotype=class_count>0 and interface and K('interface') or nil}
    for _,group in ipairs{{impls,'implements'},{inherits,'inheritance'},{requires,'dependency'},{external,'dependency'}} do
      for _,to in ipairs(group[1]) do if not E.equal(m.id,to) then unique(edges,record{from=m.id,to=to,kind=K(group[2])}) end end
    end
  end
  for _,id in ipairs(foreigns) do if not project[id.name] then classes[#classes+1]=G.foreign_class(id) end end
  return record{classes=classes,edges=edges}
end
return M
