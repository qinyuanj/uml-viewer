local lfs=require('lfs')
local ranks={edn=0,compat=0,domain=0,source=0,graph=0,typescript_language=0,rust_language=0,python_language=0,lua_language=0,engine=1,application=2,adapters=3,main=4}
local function sources(root,result)
  result=result or {}
  for name in lfs.dir(root) do
    if name ~= '.' and name ~= '..' then
      local path=root..'/'..name
      if lfs.attributes(path,'mode') == 'directory' then sources(path,result)
      elseif name:match('%.lua$') then result[#result+1]=path end
    end
  end
  return result
end
local function check(path)
  local file=assert(io.open(path)); local text=file:read('*a'); file:close()
  local owner=path:match('love/uml/([^/.]+)')
  assert.is_not_nil(ranks[owner],'unknown module layer: '..path)
  local rank=ranks[owner]
  if rank < 3 then
    assert.is_nil(text:match('%f[%w_]love%s*[%.%[]'),'pure module references love: '..path)
    assert.is_nil(text:match('%f[%w_]ffi%f[^%w_]'),'pure module references FFI: '..path)
  end
  for dependency in text:gmatch('require%s*%(?%s*[\'"]([%w_%.]+)[\'"]') do
    local target=dependency:match('^uml%.([^%.]+)')
    if target then
      assert.is_not_nil(ranks[target],'unknown dependency '..dependency)
      assert.is_true(ranks[target] <= rank,'outer layer dependency: '..path..' -> '..dependency)
      if target:match('_language$') then assert.are.equal('main',owner,'language wiring must be in main') end
    end
  end
end

describe('Lua architecture',function()
  for _,path in ipairs(sources('love/uml')) do
    it('keeps module dependencies within the declared boundary: '..path,function() check(path) end)
  end
  it('loads every pure module with runtime globals unavailable',function()
    local saved_love,saved_ffi=_G.love,package.loaded.ffi
    _G.love=setmetatable({},{__index=function() error('love forbidden') end})
    package.loaded.ffi=setmetatable({},{__index=function() error('ffi forbidden') end})
    local ok,message=pcall(function()
      for _,path in ipairs(sources('love/uml')) do
        local owner=path:match('love/uml/([^/.]+)')
        if ranks[owner] < 3 then assert(loadfile(path))() end
      end
    end)
    _G.love,package.loaded.ffi=saved_love,saved_ffi
    assert.is_true(ok,message)
  end)
end)
