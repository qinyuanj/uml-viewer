#!/usr/bin/env python3
"""Create the isolated #114 Lua project (Chinese + space path)."""
import subprocess
import sys
from pathlib import Path

P = Path(sys.argv[1])
FILES = {
'app/pricing.lua': '''-- 价格规则：数量折扣。
local M={}
function M.unit_price(base,quantity)
  if quantity>=100 then return base*0.8
  elseif quantity>=10 then return base*0.9 end
  return base
end
function M.total(base,quantity) return M.unit_price(base,quantity)*quantity end
return M
''',
'app/inventory/store.lua': '''-- 库存存储：按 SKU 计数。
local M={}
function M.new() return {items={}} end
function M.add(store,sku,count)
  store.items[sku]=(store.items[sku] or 0)+count
  return store.items[sku]
end
function M.remove(store,sku,count)
  local have=store.items[sku] or 0
  if count>have then return nil,'库存不足' end
  store.items[sku]=have-count
  return store.items[sku]
end
function M.count(store,sku) return store.items[sku] or 0 end
return M
''',
'app/inventory/init.lua': '''-- 库存服务：出库并计价。
local Store=require('app.inventory.store')
local Pricing=require('app.pricing')
local M={}
function M.new() return {store=Store.new()} end
function M.receive(service,sku,count) return Store.add(service.store,sku,count) end
function M.ship(service,sku,count,base)
  local left,reason=Store.remove(service.store,sku,count)
  if not left then return nil,reason end
  return Pricing.total(base,count),left
end
return M
''',
'spec/inventory_spec.lua': '''local Inventory=require('app.inventory')
local Pricing=require('app.pricing')
describe('库存',function()
  it('出库计价',function()
    local s=Inventory.new(); Inventory.receive(s,'a',20)
    local price,left=Inventory.ship(s,'a',10,2)
    assert.are.equal(18,price); assert.are.equal(10,left)
  end)
  it('库存不足',function()
    local s=Inventory.new()
    local price,reason=Inventory.ship(s,'a',1,2)
    assert.is_nil(price); assert.are.equal('库存不足',reason)
  end)
  it('批量折扣',function() assert.are.equal(80,Pricing.total(1,100)) end)
end)
''',
'coverage.lua': '''-- 项目覆盖率入口（crapper/mutator 的 --coverage-command）；luacov 位于用户 LuaRocks 树。
package.path='./?.lua;./?/init.lua;'..package.path
local function local_rocks()
  local home=os.getenv('HOME'); local v=_VERSION:match('%d+%.%d+')
  package.path=home..'/.luarocks/share/lua/'..v..'/?.lua;'..home..'/.luarocks/share/lua/'..v..'/?/init.lua;'..package.path
  package.cpath=home..'/.luarocks/lib/lua/'..v..'/?.so;'..package.cpath
end
if not pcall(require,'luacov.runner') then local_rocks() end
local lfs=require('lfs')
assert(lfs.attributes('coverage','mode')=='directory' or lfs.mkdir('coverage'))
local ok,version=pcall(require,'cluacov.version')
if not ok or version~='1.0.0' then io.stderr:write('需要 cluacov 1.0.0\\n'); os.exit(1) end
local runner=require('luacov.runner')
local config=runner.load_config('coverage_config.lua')
if arg[1]=='report' then config.reporter='lcov'; runner.run_report(config)
else
  os.remove('target/coverage/lua/lcov.info'); os.remove('target/coverage/lua/luacov.stats.out')
  runner.init(config); arg={'spec'}; require('busted.runner')({standalone=false}); runner.shutdown()
end
''',
'coverage_config.lua': '''local lfs=require('lfs')
local files,modules={},{}
local function walk(dir)
  for name in lfs.dir(dir) do if name~='.' and name~='..' then
    local path=dir..'/'..name
    if lfs.attributes(path,'mode')=='directory' then walk(path)
    elseif name:match('%.lua$') then
      files[#files+1]=path
      local stem=path:gsub('%.lua$','')
      modules[stem]=path
      modules[(stem:gsub('/init$',''):gsub('/','.'))]=path
      modules[(stem:gsub('/','.'))]=path
    end
  end end
end
walk('app'); table.sort(files)
return {statsfile='coverage/luacov.stats.out',reportfile='coverage/lcov.info',
  include={'^app/'},exclude={'/spec/','^%.uml%-viewer/'},modules=modules,includeuntestedfiles=files}
''',
'uml-quality.json': '''{
  "version": 1,
  "standardTools": {
    "sourceRoots": ["app"],
    "exclude": [],
    "crapperOptions": ["--coverage-command", "lua5.4 coverage.lua && lua5.4 coverage.lua report"],
    "mutatorOptions": ["--max-workers", "2", "--coverage-command", "lua5.4 coverage.lua && lua5.4 coverage.lua report"]
  }
}
''',
'policy.edn': '''{:title "库存 项目"
 :src "app"
 :prefix "app"
 :lang :lua
 :hierarchical true
 :proposals [{:id :p-1792500000000, :name "出库提案", :layers []}]
 :out "inventory.edn"}
''',
'.gitignore': 'coverage/\ntarget/\n',
}
for name, text in FILES.items():
    path = P / name
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)
subprocess.run(['git', 'init', '-q'], cwd=P, check=True)
subprocess.run(['git', 'add', '-A'], cwd=P, check=True)
subprocess.run(['git', '-c', 'user.name=uml-114', '-c', 'user.email=uml-114@localhost',
                'commit', '-qm', 'isolated #114 project'], cwd=P, check=True)
print(P)
