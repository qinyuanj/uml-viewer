-- Composition root: language modules are registered explicitly here.
local E=require('uml.edn')
local G=require('uml.graph')
local IR=require('uml.application.ir_generator')
local Files=require('uml.adapters.project_files')
local Lua=require('uml.lua_language.graph_lua')
local M={}
local function get(m,k,d) local v=E.get(m,E.keyword(k)); if v==nil or v==E.nil_value or v==false then return d end; return v end
function M.run(args)
  assert(love.window==nil and love.graphics==nil,'ir must run without window or graphics modules')
  local root=love.filesystem.getWorkingDirectory(); local positional={}; local i=1
  while i<=#args do
    local a=args[i]
    if a=='--root' then i=i+1; assert(args[i],'--root requires a path'); root=args[i]
    elseif a=='--policy' or a=='--output' then i=i+1; assert(args[i],a..' requires a path'); local n=a=='--policy' and 1 or 2; assert(not positional[n],'duplicate '..a); positional[n]=args[i]
    elseif a:sub(1,2)=='--' then error('unknown ir option: '..a)
    else local n=positional[1] and 2 or 1; assert(not positional[n],'usage: ir [policy [output]] [--root directory]'); positional[n]=a end
    i=i+1
  end
  assert(love.system.getOS()=='Windows','IR native path adapter currently supports Windows')
  local paths=require('uml.adapters.windows_path'); root=paths.canonical(root)
  local files=Files.new(root,paths)
  local function register(lang,scanner)
    G.register(lang,function(source,opts) local canonical,snapshot=files.snapshot(source,scanner,opts); return scanner.scan(canonical,opts,snapshot,root) end)
  end
  register('lua',Lua)
  local path=positional[1] or 'examples/uml-viewer.policy.edn'
  local p=IR.read_policy(path,files); local lang=get(p,'lang',E.keyword('clojure')); local impl=G.lookup(lang)
  if not impl and #get(p,'sources',E.vector())==0 then error('no LanguageGraph for :'..lang.name) end
  local out=IR.generate(impl,path,positional[2] or get(p,'out','examples/uml-viewer.edn'),files)
  print('Wrote '..out); return 0
end
return M
