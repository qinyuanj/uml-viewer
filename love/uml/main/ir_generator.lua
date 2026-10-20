-- Composition root: language modules are registered explicitly here.
local E=require('uml.edn')
local G=require('uml.graph')
local IR=require('uml.application.ir_generator')
local Files=require('uml.adapters.project_files')
local scanners={
  lua='uml.lua_language.graph_lua',
  typescript='uml.typescript_language.graph_typescript',
  rust='uml.rust_language.graph_rust',
  python='uml.python_language.graph_python'
}
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
  local os_name=love.system.getOS()
  assert(os_name=='Linux' or os_name=='OS X','IR requires WSL or macOS Apple Silicon')
  local paths=require('uml.adapters.posix_path'); root=paths.canonical(root)
  local files=Files.new(root,paths)
  for lang,module in pairs(scanners) do
    local selected=module
    G.register(lang,function(source,opts)
      local ok,scanner=pcall(require,selected)
      if not ok then error('Scanner unavailable: '..selected..': '..tostring(scanner),0) end
      local canonical,snapshot=files.snapshot(source,scanner,opts)
      return scanner.scan(canonical,opts,snapshot,root)
    end)
  end
  local path=positional[1] or 'examples/uml-viewer.policy.edn'
  local p=IR.read_policy(path,files)
  local out=IR.generate(nil,path,positional[2] or get(p,'out','examples/uml-viewer.edn'),files)
  print('Wrote '..out); return 0
end
return M
