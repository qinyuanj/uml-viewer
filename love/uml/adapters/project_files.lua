-- Native enumeration adapter; scanners receive immutable snapshots.
local Atomic=require('uml.adapters.atomic_file')
local M={}
function M.new(root,host)
  local function absolute(path)
    if path:match('^%a:[/\\]') or path:match('^[/\\]') then return path end
    return root..'/'..path
  end
  local function read(path)
    local f,err=io.open(absolute(path),'rb'); assert(f,err); local text=f:read('*a'); f:close(); return text
  end
  local function write(path,text)
    Atomic.write(absolute(path),text)
  end
  local function snapshot(path,scanner,opts)
    local canonical=host.canonical(absolute(path)); local files={}
    local function walk(relative)
      for _,entry in ipairs(host.entries(canonical..(relative=='' and '' or '/'..relative))) do
        local rel=relative=='' and entry.name or relative..'/'..entry.name
        if entry.directory then
          if not scanner.accepts_directory or scanner.accepts_directory(rel,opts) then walk(rel) end
        elseif scanner.accepts(rel,opts) then
          local real=host.canonical(canonical..'/'..rel); files[#files+1]={path=real,source=read(real)}
        end
      end
    end
    walk('')
    if scanner.metadata_paths then
      local seen={}; for _,file in ipairs(files) do seen[file.path]=true end
      for _,path in ipairs(scanner.metadata_paths(canonical)) do
        local f=io.open(path,'rb')
        if f then
          local text=f:read('*a'); f:close(); local real=host.canonical(path)
          if not seen[real] then files[#files+1]={path=real,source=text}; seen[real]=true end
        end
      end
    end
    return canonical,files
  end
  local function filenames()
    local names={}
    local function walk(path)
      for _,entry in ipairs(host.entries(root..(path=='' and '' or '/'..path))) do
        local rel=path=='' and entry.name or path..'/'..entry.name
        if entry.directory then
          if entry.name~='.git' and entry.name~='node_modules' and entry.name~='.uml-viewer' and entry.name~='.claude' then walk(rel) end
        else names[#names+1]=rel end
      end
    end
    walk(''); return names
  end
  return {read=read,write=write,snapshot=snapshot,filenames=filenames,warn=function(s) io.stderr:write(s,'\n') end}
end
return M
