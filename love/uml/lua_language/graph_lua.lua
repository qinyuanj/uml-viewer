-- Pure Lua module scanner; source snapshots and canonical paths come from callers.
local E=require('uml.edn')
local G=require('uml.graph')
local M={}
local K,V=E.keyword,E.vector
local function record(t) return E.from_table(t) end
local function get(m,k,d) local v=E.get(m,K(k)); if v==nil or v==E.nil_value or v==false then return d end; return v end
local function unique(xs,x) for _,v in ipairs(xs) do if E.equal(v,x) then return end end; xs[#xs+1]=x end
local function ident(c) return c~='' and c:match('[%w_]')~=nil end
local function keyword(s,i,w) return s:sub(i,i+#w-1)==w and not ident(s:sub(i-1,i-1)) and not ident(s:sub(i+#w,i+#w)) end
local function long_end(s,i)
  local eq=s:sub(i):match('^%[(=*)%['); if eq==nil then return nil end
  local _,b=s:find(']'..eq..']',i+#eq+2,true); return b and b+1 or #s+1
end
local function mask(source)
  local out,holes={},{}; local i=1
  while i<=#source do
    local c=source:sub(i,i); local finish,quoted
    if source:sub(i,i+1)=='--' then finish=long_end(source,i+2) or source:find('\n',i,true) or #source+1
    elseif c=='"' or c=="'" then finish=G.scan_quoted(source,i,c); quoted=true
    elseif c=='[' then finish=long_end(source,i) end
    if finish then
      local text=source:sub(i,finish-1)
      if quoted then out[#out+1]=text; holes[#holes+1]={i+1,math.min(#source+1,finish-1)}
      else out[#out+1]=text:gsub('[^\n]',' ') end
      i=finish
    else out[#out+1]=c; i=i+1 end
  end
  return table.concat(out),holes
end
local function in_hole(holes,i) for _,h in ipairs(holes) do if i>=h[1] and i<h[2] then return true end end; return false end
local function declaration(rest,kw)
  local name,localp
  if kw=='local' then
    name=rest:match('^local%s+function%s+([%a_][%w_%.:]*)') or rest:match('^local%s+([%a_][%w_%.:]*)%s*=%s*function%f[^%w_]')
    localp=true
  elseif kw=='function' then name=rest:match('^function%s+([%a_][%w_%.:]*)')
  elseif not kw then name=rest:match('^([%a_][%w_%.:]*)%s*=%s*function%f[^%w_]') end
  return name,localp==true
end
function M.read_module(source)
  local text,holes=mask(source); local requires,defns,bases=V(),V(),V(); local bindings,base_names={},{}; local depth=0
  local steps={['function']=1,['if']=1,['do']=1,['repeat']=1,['end']=-1,['until']=-1}
  for i=1,#text do
    if not in_hole(holes,i) then
      if keyword(text,i,'require') then
        local tail=text:sub(i+7):match('^[ \t]*(.*)')
        if tail:sub(1,1)=='(' then tail=tail:sub(2):match('^[ \t]*(.*)') end
        local quote=tail:sub(1,1)
        if quote=='"' or quote=="'" then local n=tail:sub(2):match('^([%a_][%w_%.]*)'); if n then requires[#requires+1]=n end end
      end
      local c,prev=text:sub(i,i),text:sub(i-1,i-1)
      if ident(c) and not ident(prev) and prev~='.' and prev~=':' then
        local kw; for _,word in ipairs{'function','local','if','do','repeat','end','until'} do if keyword(text,i,word) then kw=word; break end end
        local statement=text:sub(1,i-1):match('([^\n;]*)$') or ''
        if kw=='local' or not statement:match('local%s*$') then
          local n,loc=declaration(text:sub(i),kw); if n then defns[#defns+1]=record{name=n,['local']=loc,nested=depth>0} end
        end
        if depth==0 and keyword(text,i,'setmetatable') then
          local tail=text:sub(i):match('^setmetatable%s*%(%s*(.*)')
          if tail then
            local rest=tail:match('^%{%s*%}%s*,%s*(.*)') or tail:match('^[%a_][%w_%.]*%s*,%s*(.*)')
            if rest then local base=rest:match('^%{%s*__index%s*=%s*([%a_][%w_]*)%s*%}%s*%)') or rest:match('^([%a_][%w_]*)%s*%)'); if base then base_names[#base_names+1]=base end end
          end
        end
        depth=math.max(0,depth+(steps[kw] or 0))
      end
    end
  end
  local pos=1
  while true do
    local a,b,binding,mod=text:find('local%s+([%a_][%w_]*)%s*=%s*require%s*%(?%s*["\']([%a_][%w_%.]*)',pos)
    if not a then break end; if not in_hole(holes,a) then bindings[binding]=mod end; pos=b+1
  end
  for _,b in ipairs(base_names) do if bindings[b] then unique(bases,bindings[b]) end end
  return record{requires=requires,defns=defns,bases=bases}
end
local skip={lua_modules=true,spec=true,tests=true,test=true,dist=true,build=true,target=true,out=true,coverage=true}
function M.accepts(relative,opts)
  if not relative:match('%.lua$') or relative:match('_spec%.lua$') then return false end
  local excluded={}; for _,n in ipairs(get(opts,'exclude',V())) do excluded[n]=true end
  for dir in relative:gmatch('([^/]+)/') do if dir:sub(1,1)=='.' or skip[dir] or excluded[dir] then return false end end
  return true
end
function M.accepts_directory(relative,opts)
  return M.accepts(relative..'/__module.lua',opts)
end
local function module_relative(path,root)
  local rel=G.relative_path(path,root)
  for _,prefix in ipairs{'src/lua/','src/','lua/'} do if rel:sub(1,#prefix)==prefix then rel=rel:sub(#prefix+1) end end
  rel=rel:gsub('%.lua$',''); if rel=='init' then rel='' else rel=rel:gsub('/init$','') end
  return rel:gsub('/','.')
end
function M.scan(root,opts,files,cwd)
  assert(type(cwd)=='string','canonical working directory required'); opts=opts or E.map()
  local prefix=get(opts,'prefix','app'); local ns_prefix=get(opts,'ns-prefix',prefix)
  local metas,by_rel,by_ns={},{},{}
  local sorted={}; for _,f in ipairs(files) do if M.accepts(G.relative_path(f.path,root),opts) then sorted[#sorted+1]=f end end
  table.sort(sorted,function(a,b) return a.path<b.path end)
  for _,f in ipairs(sorted) do
    local rel=module_relative(f.path,root); local ns
    if rel==ns_prefix or rel:sub(1,#ns_prefix+1)==ns_prefix..'.' then ns=rel else ns=G.ns_join(ns_prefix,rel) end
    local cid=G.id_of(ns,prefix); local m={file=f,relative=rel,ns=ns,id=cid}; metas[#metas+1]=m
    if rel~='' then by_rel[rel]=cid end; if ns~='' then by_ns[ns]=cid end
  end
  local classes,edges,foreigns=V(),V(),V()
  for _,m in ipairs(metas) do
    local surface=M.read_module(m.file.source); local ops=V()
    for _,d in ipairs(get(surface,'defns')) do if not get(d,'local') and not get(d,'nested') then ops[#ops+1]=record{name=get(d,'name'),text=get(d,'name')} end end
    classes[#classes+1]=record{id=m.id,ns=m.ns,name=G.module_name(m.id),lang=K('lua'),file=G.relative_path(m.file.path,cwd),ops=ops}
    local function edge(to,kind) if not E.equal(m.id,to) then unique(edges,record{from=m.id,to=to,kind=K(kind)}) end end
    for _,n in ipairs(get(surface,'bases')) do local cid=by_rel[n] or by_ns[n]; if cid then edge(cid,'inheritance') end end
    for _,n in ipairs(get(surface,'requires')) do local cid=by_rel[n] or by_ns[n]; if not cid then cid=K(n); unique(foreigns,cid) end; edge(cid,'dependency') end
  end
  for _,cid in ipairs(foreigns) do classes[#classes+1]=G.foreign_class(cid) end
  return record{classes=classes,edges=edges}
end
return M
