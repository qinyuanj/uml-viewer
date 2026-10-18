-- LanguageSource contract. Filesystem operations are supplied by an adapter.
local M={}
local languages={}
function M.register(lang,impl) languages[lang]=impl; return lang end
function M.lookup(lang) return languages[lang] end
local function word(c) return c~='' and c:match('[%w_]')~=nil end
function M.pattern_start(text,name,groups)
  if not text or not name then return nil end
  local quoted=tostring(name):gsub('([^%w])','%%%1')
  for _,group in ipairs(groups) do
    local best
    for _,fmt in ipairs(type(group)=='table' and group or {group}) do
      local pattern=fmt:gsub('@',function() return quoted end)
      local from=1
      while from<=#text+1 do
        local first,last=text:find(pattern,from)
        if not first then break end
        -- Java regex \b compares the two sides, including non-word names.
        if word(text:sub(last,last))~=word(text:sub(last+1,last+1)) then
          if not best or first<best then best=first end
          break
        end
        from=first+1
      end
    end
    if best then return best end
  end
end
function M.line_number(text,start) local _,n=text:sub(1,start-1):gsub('\n',''); return n+1 end
function M.declaration_line(text,start)
  local last=text:find('\n',start,true) or (#text+1)
  return (text:sub(start,last-1):gsub('%s+$',''))
end
function M.file_source(patterns,attribute)
  local impl={}
  function impl.locate(ident,files)
    local path=ident.file
    if type(path)~='string' or path=='' then return nil end
    path=path:gsub('\\','/')
    if files.root and not path:match('^%a:') and path:sub(1,1)~='/' then path=files.root:gsub('\\','/'):gsub('/$','')..'/'..path end
    if files.is_file(path) then return path end
  end
  function impl.extract(text,ident)
    local first=M.pattern_start(text,ident.name,patterns)
    if not first then return nil end
    if attribute then
      local name_at=text:find(ident.name,first,true) or first
      local last=text:find('\n',name_at,true) or (#text+1)
      return (text:sub(first,last-1):gsub('%s+$',''))
    end
    return M.declaration_line(text,first)
  end
  function impl.start_line(text,ident)
    local first=M.pattern_start(text,ident.name,patterns)
    return first and M.line_number(text,first) or nil
  end
  function impl.title(ident) return (ident.file or ident.ns or '')..(ident.name and '/'..ident.name or '') end
  return impl
end
function M.member_source(ident,files,impl)
  impl=impl or M.lookup(ident.lang)
  if not impl then return nil end
  local path=impl.locate(ident,files)
  if not path then return nil end
  local body=files.read(path)
  if body==nil then return nil end
  local named=ident.name~=nil and tostring(ident.name)~=''
  if named and not impl.extract(body,ident) then return nil end
  local line=named and impl.start_line(body,ident) or nil
  return {file=path,body=body,line=line,lang=ident.lang,title=named and (path..':'..(line or 1)) or path}
end
return M
