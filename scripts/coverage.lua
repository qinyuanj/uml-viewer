-- Invoked by crapper's documented --coverage-command through public ./uml.
-- Same executable handles collection and conversion, so no shell/LuaRocks argv
-- reconstruction is needed. Never invent or default coverage values.
package.path='love/?.lua;love/?/init.lua;'..package.path
-- luacov may live in the user's local LuaRocks tree instead of the system one.
-- Resolve it here so the documented coverage command needs no shell profile.
local function local_rocks()
  local home=os.getenv('HOME')
  if not home then return end
  for _,interpreter in ipairs{'5.1','5.2','5.3','5.4'} do
    package.path=home..'/.luarocks/share/lua/'..interpreter..'/?.lua;'
      ..home..'/.luarocks/share/lua/'..interpreter..'/?/init.lua;'..package.path
    package.cpath=home..'/.luarocks/lib/lua/'..interpreter..'/?.so;'..package.cpath
  end
end
if not pcall(require,'luacov.runner') then local_rocks() end
local lfs=require('lfs')
assert(lfs.attributes('coverage','mode')=='directory' or lfs.mkdir('coverage'))
local runner=require('luacov.runner')
local config=runner.load_config('scripts/coverage_config.lua')
if arg[1]=='report' then
  config.reporter='lcov'
  runner.run_report(config)
else
  -- These are the default Lua runner's generated inputs. Mutator also discovers
  -- them during load_bundle; an older managed report must not override this
  -- invocation's fresh custom report. Findings snapshots are never removed.
  os.remove('target/coverage/lua/lcov.info')
  os.remove('target/coverage/lua/luacov.stats.out')
  runner.init(config)
  arg={'love/spec'}
  require('busted.runner')({standalone=false})
  runner.shutdown()
end
