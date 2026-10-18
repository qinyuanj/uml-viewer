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
-- Mutation workers run deep below target/mutation-workers on Windows; the
-- relative path resolves to an absolute path past MAX_PATH, which plain
-- io.open rejects. The \\?\ prefix opens the same file without that limit.
local function open_path(path)
  if package.config:sub(1,1) ~= '\\' then return path end
  if path:sub(1,4) == '\\\\?\\' then return path end
  return '\\\\?\\' .. lfs.currentdir() .. '\\' .. (path:gsub('/', '\\'))
end

describe('repository EDN compatibility',function()
  for _, root in ipairs({'examples','.metrics','lua-fixture','acceptance-evidence','fixtures/jvm-goldens','typescript-fixture','rust-fixture','python-fixture'}) do
    for _, path in ipairs(files(root)) do
      it('preserves semantics of '..path,function()
        local f=assert(io.open(open_path(path),'rb')); local source=f:read('*a'); f:close()
        local value=edn.parse(source)
        assert.is_true(edn.equal(value,edn.parse(edn.encode(value))))
      end)
    end
  end
end)
