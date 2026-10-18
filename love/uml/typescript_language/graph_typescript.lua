-- Pure port of the JVM TypeScript regex/string scanner. Callers own IO and paths.
local E=require('uml.edn')
local G=require('uml.graph')
local M={}
local K,V=E.keyword,E.vector
local function record(t) local m=E.map(); for k,v in pairs(t) do E.assoc(m,K(k),v) end; return m end
local function get(m,k,d) local v=E.get(m,K(k)); if v==nil or v==E.nil_value or v==false then return d end; return v end
local function unique(xs,x) for _,v in ipairs(xs) do if E.equal(v,x) then return end end; xs[#xs+1]=x end
local ID='[A-Za-z_$][%w_$]*'
local function brace_names(stmt)
  local names=V(); local body=stmt:match('{([^}]*)}')
  for part in (body or ''):gmatch('[^,]+') do part=part:match('^%s*(.-)%s*$'):gsub('^type%s+',''); local n=part:match('%f[%w_$]as%s+('..ID..')%s*$') or part:match('^('..ID..')'); if n then names[#names+1]=n end end
  return names
end
local function ident(c) return c~='' and c:match('[%w_$]')~=nil end
local regex_keywords={['return']=true,throw=true,case=true,void=true,typeof=true,delete=true,await=true,yield=true,['else']=true,['do']=true,['in']=true,of=true}
local function regex_end(s,i)
  local j=i-1; while j>0 and s:sub(j,j):match('%s') do j=j-1 end
  local c=s:sub(j,j)
  if j>0 and (ident(c) or c==')' or c==']') then
    local k=j; while k>0 and ident(s:sub(k,k)) do k=k-1 end
    if not regex_keywords[s:sub(k+1,j)] then return end
  end
  local inside=false; j=i+1
  while j<=#s do
    c=s:sub(j,j)
    if c=='\\' then j=j+2
    elseif c=='\n' then return
    elseif c=='/' and not inside then j=j+1; while s:sub(j,j):match('%a') do j=j+1 end; return j
    else if c=='[' and not inside then inside=true elseif c==']' and inside then inside=false end; j=j+1 end
  end
end
local function mask(s)
  local out,holes={},{}; local i=1
  while i<=#s do
    local c,nxt=s:sub(i,i),s:sub(i+1,i+1); local finish
    if c=='/' and nxt=='/' then i=G.mask_line_comment(s,out,i)
    elseif c=='/' and nxt=='*' then i=G.mask_block_comment(s,out,i)
    else
      if c=='/' and nxt~='' and nxt~='*' then finish=regex_end(s,i)
      elseif c=='"' or c=="'" or c=='`' then finish=G.scan_quoted(s,i,c) end
      if finish then out[#out+1]=s:sub(i,finish-1); holes[#holes+1]={i,finish}; i=finish
      else out[#out+1]=c; i=i+1 end
    end
  end
  return table.concat(out),holes
end
local function in_hole(holes,i) for _,h in ipairs(holes) do if i>=h[1] and i<h[2] then return true end end; return false end
local function statement_end(s,start,header)
  local paren,brace,bracket=0,0,0
  for i=start,#s do
    local c=s:sub(i,i)
    if c=='(' then paren=paren+1 elseif c==')' then paren=math.max(0,paren-1)
    elseif c=='[' then bracket=bracket+1 elseif c==']' then bracket=math.max(0,bracket-1)
    elseif c=='{' then if header and paren==0 and bracket==0 and brace==0 then return i-1 end; brace=brace+1
    elseif c=='}' and not header then brace=math.max(0,brace-1)
    elseif paren==0 and bracket==0 and brace==0 then
      if header and (c=='=' or c==';') then return i-1 elseif c==';' then return i end
    end
  end
  return #s
end
local function from_spec(stmt) return stmt:match('from%s*["\']([^"\']+)["\']') end
local function return_type(header)
  local open=header:find('(',1,true); if not open then return end
  local depth=1
  for i=open+1,#header do local c=header:sub(i,i); if c=='(' then depth=depth+1 elseif c==')' then depth=depth-1; if depth==0 then return header:sub(i+1):match('^%s*:%s*('..ID..')%s*$') end end end
end
local function export_fact(stmt,header)
  if stmt:match('^export%s+type%s*{') or stmt:match('^export%s+%*') then return record{spec=from_spec(stmt) or E.nil_value} end
  if stmt:match('^export%s+{') then return record{spec=from_spec(stmt) or E.nil_value,ops=brace_names(stmt)} end
  local tail=stmt:gsub('^export%s+',''):gsub('^default%s+','')
  if tail:match('^async%s+') then tail=tail:gsub('^async%s+',''); if not tail:match('^function%f[^%w_$]') then return end end
  local n,typ
  if tail:match('^function%f[^%w_$]') then n=tail:match('^function%s+%*?%s*('..ID..')'); typ=return_type(header)
  elseif tail:match('^class%f[^%w_$]') then n=tail:match('^class%s+('..ID..')')
  elseif tail:match('^interface%f[^%w_$]') then return record{interface=true}
  elseif tail:match('^enum%f[^%w_$]') then n=tail:match('^enum%s+('..ID..')')
  elseif stmt:match('^export%s+const%f[^%w_$]') or stmt:match('^export%s+let%f[^%w_$]') or stmt:match('^export%s+var%f[^%w_$]') then
    n=tail:match('^%a+%s+('..ID..')'); typ=header:match('%f[%w_$]const%s+'..ID..'%s*:%s*('..ID..')%s*$') or header:match('%f[%w_$]let%s+'..ID..'%s*:%s*('..ID..')%s*$') or header:match('%f[%w_$]var%s+'..ID..'%s*:%s*('..ID..')%s*$')
  end
  if n then return record{ops=V{n},type=typ,value=true} end
end
function M.read_module(source)
  local text,holes=mask(source); local imports,exports,invokes=V(),V(),V()
  -- Java multiline ^\\s* matches may begin on an earlier blank line. Preserve
  -- that match start for hole rejection, rather than testing the keyword start.
  for _,word in ipairs{'import','export'} do
    local start=1
    while start<=#text do
      local a,b=text:find('^%s*'..word..'%f[^%w_$]',start)
      if a then
        if not in_hole(holes,a) then
          local at=text:find(word,a,true); local stmt=text:sub(at,statement_end(text,at)); local header=text:sub(at,statement_end(text,at,true))
          if word=='import' then
            local spec=from_spec(stmt) or stmt:match('^import%s*["\']([^"\']+)["\']')
            if spec then
              local names=brace_names(stmt); local tail=stmt:gsub('^import%s+',''):gsub('^type%s+','')
              local n,rest=tail:match('^('..ID..')%s*(.*)')
              if n and n~='type' and (rest:sub(1,1)==',' or tail:match('^'..ID..'%s+from%f[^%w_$]')) then names[#names+1]=n end
              n=stmt:match('%*%s+as%s+('..ID..')'); if n then names[#names+1]=n end
              imports[#imports+1]=record{spec=spec,names=names}
            end
          else local fact=export_fact(stmt,header); if fact then exports[#exports+1]=fact end end
        end
        start=b+1
      end
      local newline=text:find('\n',start,true); if not newline then break end; start=newline+1
    end
  end
  local i=1
  while true do
    local at=text:find('invoke',i,true); if not at then break end; i=at+1
    if not ident(text:sub(at-1,at-1)) and not ident(text:sub(at+6,at+6)) and not in_hole(holes,at) then
      local j=at+6; while text:sub(j,j):match('%s') do j=j+1 end
      if text:sub(j,j)=='<' then
        local depth=1; j=j+1
        while j<=#text and depth>0 do local c=text:sub(j,j); if c=='<' then depth=depth+1 elseif c=='>' then depth=depth-1 end; j=j+1 end
      end
      if text:sub(j,j)=='(' then
        j=j+1; while text:sub(j,j):match('%s') do j=j+1 end
        local quote=text:sub(j,j)
        if quote=='"' or quote=="'" then
          local stop=j+1
          while stop<=#text do local c=text:sub(stop,stop); if c=='\\' then stop=stop+2 elseif c==quote then break else stop=stop+1 end end
          unique(invokes,text:sub(j+1,math.min(stop,#text+1)-1))
        end
      end
    end
  end
  return record{imports=imports,exports=exports,invokes=invokes}
end
function M.accepts(relative)
  return relative:match('%.[cm]?[jt]sx?$')~=nil and not relative:match('%.d%.ts$') and not relative:match('%.test%.[cm]?[jt]sx?$') and not relative:match('%.spec%.[cm]?[jt]sx?$') and not ('/'..relative):match('/node_modules/') and not ('/'..relative):match('/dist/') and not ('/'..relative):match('/target/')
end
local function normalize(path)
  local parts={}; for p in path:gmatch('[^/]+') do if p=='..' then table.remove(parts) elseif p~='.' then parts[#parts+1]=p end end
  return (path:sub(1,1)=='/' and '/' or '')..table.concat(parts,'/')
end
local function module_relative(path,root)
  local rel=G.relative_path(path,root):gsub('%.[cm]?[jt]sx?$','')
  if rel=='index' then rel='' else rel=rel:gsub('/index$','/') end
  return (rel:gsub('/%.','.'):gsub('/$',''):gsub('/','.'))
end
local exts={'.ts','.tsx','.js','.jsx','.mjs','.cjs'}
function M.scan(root,opts,files,cwd)
  assert(type(cwd)=='string','canonical working directory required'); opts=opts or E.map()
  local prefix=get(opts,'prefix','app'); local ns_prefix=get(opts,'ns-prefix',prefix)
  local sorted,by_path,project={},{},{}
  for _,f in ipairs(files) do if M.accepts(f.path,opts) then sorted[#sorted+1]=f end end
  table.sort(sorted,function(a,b) return a.path<b.path end)
  for _,f in ipairs(sorted) do local cid=G.id_of(G.ns_join(ns_prefix,module_relative(f.path,root)),prefix); by_path[f.path]=cid; project[E.encode(cid)]=true end
  local function resolve(file,spec)
    if not spec or spec:sub(1,1)~='.' then return end
    local base=normalize(file:match('^(.*)/')..'/'..spec); if by_path[base] then return by_path[base] end
    for _,ext in ipairs(exts) do if by_path[base..ext] then return by_path[base..ext] end end
    for _,ext in ipairs(exts) do if by_path[base..'/index'..ext] then return by_path[base..'/index'..ext] end end
  end
  local classes,edges,foreigns,invokes=V(),V(),V(),E.map()
  for _,f in ipairs(sorted) do
    local surface=M.read_module(f.source); local cid=by_path[f.path]; local ops,bindings,specs,impls=V(),{},V(),V(); local interface,value=false,false
    for _,imp in ipairs(get(surface,'imports')) do local spec=get(imp,'spec'); unique(specs,spec); local to=resolve(f.path,spec); if to then for _,n in ipairs(get(imp,'names')) do bindings[n]=to end end end
    for _,exp in ipairs(get(surface,'exports')) do for _,n in ipairs(get(exp,'ops',V())) do unique(ops,record{name=n,text=n}) end; local spec=get(exp,'spec'); if spec then unique(specs,spec) end; local typ=get(exp,'type'); if typ and bindings[typ] then unique(impls,bindings[typ]) end; interface=interface or get(exp,'interface',false); value=value or get(exp,'value',false) end
    local c=record{id=cid,name=G.module_name(cid),ns=G.ns_join(ns_prefix,module_relative(f.path,root)),lang=K('typescript'),file=G.relative_path(f.path,cwd)}
    if #ops>0 then E.assoc(c,K('ops'),ops) end; if interface and not value then E.assoc(c,K('stereotype'),K('interface')) end; classes[#classes+1]=c
    local function edge(to,kind) if not E.equal(cid,to) then unique(edges,record{from=cid,to=to,kind=K(kind)}) end end
    local requires,external=V(),V()
    for _,spec in ipairs(specs) do
      local ext=spec:match('%.([^.]+)$'); local assets={css=true,scss=true,sass=true,less=true,svg=true,png=true,jpg=true,jpeg=true,gif=true,webp=true,html=true,md=true,json=true}
      if not assets[ext] then local to=resolve(f.path,spec); if to then unique(requires,to) elseif spec:sub(1,1)~='.' then local foreign=K((spec:gsub('^@','at.'):gsub('[:/]','.'))); unique(external,foreign); unique(foreigns,foreign) end end
    end
    for _,to in ipairs(requires) do edge(to,'dependency') end; for _,to in ipairs(external) do edge(to,'dependency') end; for _,to in ipairs(impls) do edge(to,'implements') end
    if #get(surface,'invokes')>0 then E.assoc(invokes,cid,get(surface,'invokes')) end
  end
  for _,cid in ipairs(foreigns) do if not project[E.encode(cid)] then classes[#classes+1]=G.foreign_class(cid) end end
  return record{classes=classes,edges=edges,invokes=invokes}
end
return M
