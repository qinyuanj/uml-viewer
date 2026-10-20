-- Shared language graph utilities. Graphs/options are uml.edn maps/vectors.
-- register(lang, scanner) injects a function(root, opts), or object:scan(root,opts).
-- scan_project requires an explicit language; language detection belongs to the caller.
-- Filesystem canonicalization is supplied by the caller to relative_path.
local E=require('uml.edn')
local M={}; local languages={}
local function lang_key(lang) return E.kind(lang)=='keyword' and lang.name or lang end
function M.register(lang,scanner) languages[lang_key(lang)]=scanner; return lang end
function M.lookup(lang) return languages[lang_key(lang)] end
function M.scan_project(lang,root,opts)
  local scanner=M.lookup(lang); if not scanner then error('no LanguageGraph for :'..tostring(lang_key(lang)),0) end
  if type(scanner)=='function' then return scanner(root,opts or E.map()) end
  return scanner:scan(root,opts or E.map())
end
local function get(m,k) if E.kind(m)=='map' then local v=E.get(m,E.keyword(k)); if v~=E.nil_value then return v end end end
local function seq(x) return E.kind(x)=='vector' and x or {} end
local function record(t) return E.from_table(t) end
function M.merge_scans(scans)
  local classes,edges=E.vector(),E.vector(); local by_id,owners={},{}
  for _,scan in ipairs(scans) do
    for _,c in ipairs(seq(get(scan,'classes'))) do
      local key=E.encode(get(c,'id')); local at=by_id[key]
      if not at then classes[#classes+1]=c; by_id[key]=#classes
      elseif not get(c,'foreign') then
        if not get(classes[at],'foreign') then error('duplicate class id: '..key,0) end; classes[at]=c
      end
    end
    for _,entry in ipairs(get(scan,'commands') or {}) do for _,cmd in ipairs(seq(entry[2])) do owners[E.encode(cmd)]=entry[1] end end
  end
  local function add(e) for _,old in ipairs(edges) do if E.equal(old,e) then return end end; edges[#edges+1]=e end
  for _,scan in ipairs(scans) do for _,e in ipairs(seq(get(scan,'edges'))) do add(e) end end
  for _,scan in ipairs(scans) do for _,entry in ipairs(get(scan,'invokes') or {}) do for _,command in ipairs(seq(entry[2])) do
    local to=owners[E.encode(command)]; if to and not E.equal(to,entry[1]) then add(record{from=entry[1],to=to,kind=E.keyword('dependency')}) end
  end end end
  return record{classes=classes,edges=edges}
end
local function name(x) if type(x)=='string' then return x end; return x.name:match('[^/]+$') end
local function blank(s) return s==nil or s==E.nil_value or s==false or s:match('^%s*$') end
function M.id_of(ns,prefix)
  prefix=prefix~=E.nil_value and prefix or nil
  if not blank(prefix) and ns:sub(1,#prefix+1)==prefix..'.' then ns=ns:sub(#prefix+2) end
  return E.keyword(ns)
end
function M.module_name(cid)
  local segment=name(cid):match('([^.]+)%.?$') or ''; local words={}
  for part in segment:gmatch('[^-]+') do
    words[#words+1]=part:sub(1,1):upper()..(part:find('[A-Z]') and part:sub(2) or part:sub(2):lower())
  end; return table.concat(words)
end
function M.relative_path(path,cwd)
  assert(type(cwd)=='string','canonical working directory required'); path=path:gsub('\\','/'); cwd=cwd:gsub('\\','/')
  local prefix=cwd..'/'; if path:sub(1,#prefix)==prefix then return path:sub(#prefix+1) end; return path
end
function M.file_base(path) if path==nil or path==E.nil_value then return nil end; return path:gsub('\\','/'):match('([^/]+)/?$') end
function M.ns_join(prefix,relative) if blank(relative) then return prefix or '' end; if blank(prefix) then return relative end; return prefix..'.'..relative end
function M.foreign_class(cid) return record{id=cid,name=name(cid),ns=name(cid),foreign=true} end
function M.member_edges(c,pairs)
  local edges=E.vector(); for _,pair in ipairs(pairs) do local targets=E.get(c,pair[1]); for _,to in ipairs(seq(targets)) do edges[#edges+1]=record{from=get(c,'id'),to=to,kind=pair[2]} end end; return edges
end
-- Text helpers use 1-based byte positions; end positions are exclusive (#source+1
-- at EOF). Output buffers/holes are ordinary Lua arrays owned by the scanner.
function M.next_char(source,i) if i<#source then return source:sub(i+1,i+1) end end
function M.line_comment(c,nxt) return c=='/' and nxt=='/' end
function M.block_comment(c,nxt) return c=='/' and nxt=='*' end
function M.mask_line_comment(source,out,i,n)
  local finish=source:find('\n',i,true) or ((n or #source)+1)
  out[#out+1]=string.rep(' ',finish-i); return finish
end
function M.mask_block_comment(source,out,i,n)
  local limit=(n or #source)+1; local close=source:find('*/',i+2,true); local finish=close and math.min(limit,close+2) or limit
  out[#out+1]=source:sub(i,finish-1):gsub('[^\n]',' '); return finish
end
function M.copy_span(out,source,holes,i,finish)
  out[#out+1]=source:sub(i,finish-1); local copied={}; for j,h in ipairs(holes) do copied[j]=h end; copied[#copied+1]={i,finish}
  return {i=finish,holes=copied}
end
function M.scan_quoted(source,i,quote,n)
  local j,escaped=i+1,false; n=n or #source
  while j<=n do local c=source:sub(j,j)
    if escaped then escaped=false elseif c=='\\' then escaped=true elseif c==quote then return j+1 end; j=j+1
  end; return n+1
end
return M
