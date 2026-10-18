local edn=require('uml.edn')
local lfs=require('lfs')
local function files(root, result)
  result=result or {}
  for name in lfs.dir(root) do
    if name ~= '.' and name ~= '..' then
      local path=root..'/'..name
      if lfs.attributes(path,'mode') == 'directory' then files(path,result)
      elseif name:match('%.edn$') then result[#result+1]=path end
    end
  end
  return result
end

describe('repository EDN compatibility',function()
  for _, root in ipairs({'examples','.metrics','lua-fixture','acceptance-evidence'}) do
    for _, path in ipairs(files(root)) do
      it('preserves semantics of '..path,function()
        local f=assert(io.open(path,'rb')); local source=f:read('*a'); f:close()
        local value=edn.parse(source)
        assert.is_true(edn.equal(value,edn.parse(edn.encode(value))))
      end)
    end
  end
end)
