-- Invoked by crapper's documented --coverage-command through public ./uml.
-- Same executable handles collection and conversion, so no shell/LuaRocks argv
-- reconstruction is needed. Never invent or default coverage values.
package.path='love/?.lua;love/?/init.lua;'..package.path
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
