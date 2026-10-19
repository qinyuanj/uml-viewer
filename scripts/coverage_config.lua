-- Whole declared business tree, including untested files. LuaCov's module
-- mapping joins require() names and filesystem slash aliases before reporting.
local lfs=require('lfs')
local files,modules={},{}
local function walk(dir)
  for name in lfs.dir(dir) do if name~='.' and name~='..' then
    local path=dir..'/'..name
    if lfs.attributes(path,'mode')=='directory' then walk(path)
    elseif name:match('%.lua$') then
      files[#files+1]=path
      local stem=path:gsub('%.lua$','')
      modules[stem]=path
      modules[stem:gsub('^love/',''):gsub('/','.')]=path
    end
  end end
end
walk('love/uml')
table.sort(files)
return {statsfile='coverage/luacov.stats.out',reportfile='coverage/lcov.info',
  include={'^love/uml/'},exclude={'/spec/','^%.uml%-viewer/'},
  modules=modules,includeuntestedfiles=files}
