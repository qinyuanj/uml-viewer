-- Filesystem boundary. Pure overlay receives immutable EDN snapshot records.
local E=require('uml.edn')
local M={}
local function parent(path) return path:match('^(.*)/[^/]+$') end
function M.new(host)
  if not host then
    local paths=require('uml.adapters.platform_path').load()
    local watch=require('uml.adapters.platform_watch').load()
    host={canonical=paths.canonical,entries=paths.entries,mtime=watch.mtime,cwd=function() return paths.canonical('.') end,
      read=function(path) local f=assert(io.open(path,'rb')); local t=f:read('*a'); f:close(); return t end}
  end
  local function root(path)
    local dir=host.canonical(path):gsub('\\','/')
    local ok=pcall(host.entries,dir); if not ok then dir=parent(dir) end
    while dir and dir~='' do
      local listed,entries=pcall(host.entries,dir)
      if listed then for _,entry in ipairs(entries) do if entry.name=='.metrics' and entry.directory then return dir end end end
      local up=parent(dir); if up==dir then break end; dir=up
    end
    return host.cwd()
  end
  local function snapshot(base)
    local files={}
    local function add(path)
      local text=host.read(base..'/'..path)
      files[#files+1]={path=path,text=text,mtime=host.mtime(base..'/'..path),size=#text}
    end
    local function walk(path)
      for _,entry in ipairs(host.entries(base..'/'..path)) do
        local rel=path..'/'..entry.name
        if entry.directory then walk(rel) elseif entry.name:match('%.edn$') then add(rel) end
      end
    end
    local found,entries=pcall(host.entries,base..'/.metrics')
    if found then for _,entry in ipairs(entries) do
      if entry.name=='crap.edn' and not entry.directory then add('.metrics/crap.edn')
      elseif entry.name=='mutate' and entry.directory then walk('.metrics/mutate') end
    end end
    table.sort(files,function(a,b) return a.path<b.path end)
    local stamp=E.vector()
    for _,f in ipairs(files) do stamp[#stamp+1]=E.vector{f.path,f.mtime,f.size} end
    return files,E.encode(stamp)
  end
  return {root=root,snapshot=snapshot}
end
return M
