local E=require('uml.edn')
local function read(s) return E.parse(s) end
local function get(m,k) return E.get(m,E.keyword(k)) end
local function scanner() return require('uml.lua_language.graph_lua') end
describe('Lua graph snapshots',function()
  it('resolves namespaces, relative modules and project inheritance with foreign requires',function()
    local s=scanner().scan('C:/project/src',read('{:prefix "app"}'),{
      {path='C:/project/src/app/base.lua',source='return {}'},
      {path='C:/project/src/app/init.lua',source='local Base=require("app.base")\nlocal lfs=require("lfs")\nsetmetatable({}, {__index=Base})\nfunction M.run() end'},
      {path='C:/project/src/spec/ignored.lua',source='return {}'},
    },'C:/project')
    assert.is_true(E.equal(read('[{:from :app :to :base :kind :inheritance} {:from :app :to :base :kind :dependency} {:from :app :to :lfs :kind :dependency}]'),get(s,'edges')))
    assert.equals(3,#get(s,'classes')); assert.equals('src/app/init.lua',get(get(s,'classes')[2],'file'))
    assert.is_true(get(get(s,'classes')[3],'foreign'))
  end)
  it('keeps fixture metrics namespaces and prefix stripping',function()
    local s=scanner().scan('C:/p/src',read('{:prefix "calc" :ns-prefix ""}'),{
      {path='C:/p/src/calc/init.lua',source='require("calc.util")'},
      {path='C:/p/src/calc/util.lua',source='function M.clamp() end'},
    },'C:/p')
    assert.equals('calc',get(get(s,'classes')[1],'ns')); assert.equals('calc.util',get(get(s,'classes')[2],'ns'))
    assert.is_true(E.equal(read('[{:from :calc :to :util :kind :dependency}]'),get(s,'edges')))
  end)
  it('prefers relative resolution over namespace and keeps unmatched requires foreign',function()
    local s=scanner().scan('C:/p',read('{:prefix "app"}'),{
      {path='C:/p/caller.lua',source='require("app.x")\nrequire("missing")'},
      {path='C:/p/app/x.lua',source=''},
      {path='C:/p/x.lua',source=''},
    },'C:/p')
    assert.is_true(E.equal(read('[{:from :caller :to :x :kind :dependency} {:from :caller :to :missing :kind :dependency}]'),get(s,'edges')))
  end)
  it('resolves requires by namespace when the relative module differs',function()
    local s=scanner().scan('C:/p/src',read('{:prefix "src" :ns-prefix "src"}'),{
      {path='C:/p/src/rules/turn.lua',source=''},
      {path='C:/p/src/turn/engine.lua',source='require("src.rules.turn")'},
    },'C:/p')
    assert.is_true(E.equal(read('[{:from :turn.engine :to :rules.turn :kind :dependency}]'),get(s,'edges')))
    assert.equals(2,#get(s,'classes'))
  end)
  it('strips source directory prefixes and init while honoring exclusions',function()
    local files={}; for _,p in ipairs{'src/lua/deep/mod.lua','src/plain.lua','lua/loose/init.lua','init.lua','deep/packages/vend.lua','_trigger/x.lua','x_spec.lua','.hidden/x.lua'} do files[#files+1]={path='C:/p/'..p,source=''} end
    local s=scanner().scan('C:/p',read('{:prefix "app" :exclude ["packages" "_trigger"]}'),files,'C:/p')
    local names={}; for _,c in ipairs(get(s,'classes')) do names[#names+1]=get(c,'ns') end
    assert.same({'app','app.loose','app.deep.mod','app.plain'},names)
    assert.is_false(scanner().accepts_directory('vendor/packages',read('{:exclude ["packages"]}')))
  end)
end)
describe('Lua module surface',function()
  it('masks comments and strings while keeping require names',function()
    local s=scanner().read_module([==[-- require("no")
--[=[ require 'no' ]=]
local text = "function nope() end require('no')"
local other = [=[ function nope() end ]=]
local ok = require 'real'
]==])
    assert.is_true(E.equal(read('["real"]'),get(s,'requires')))
    assert.is_true(E.equal(read('[]'),get(s,'defns')))
  end)
  it('counts every block and closes loops and repeat only once',function()
    local s=scanner().read_module('while x do y() end\nfor i=1,2 do y() end\nrepeat y() until x\ndo local function scoped() end end\nfunction outer() local function inner() end end\nfunction after() end')
    assert.is_true(E.equal(read('[{:name "scoped" :local true :nested true} {:name "outer" :local false :nested false} {:name "inner" :local true :nested true} {:name "after" :local false :nested false}]'),get(s,'defns')))
  end)
  it('finds only top-level metatable bases bound by require',function()
    local s=scanner().read_module('local Base=require("app.base")\nlocal Mix=require "app.mix"\nlocal C=setmetatable({}, {__index=Base})\nsetmetatable(C,Mix)\nfunction C.new() return setmetatable({},Base) end\nx.setmetatable({},Base)')
    assert.is_true(E.equal(read('["app.base" "app.mix"]'),get(s,'bases')))
  end)
  it('handles EOF comments, unclosed strings, and require lookahead',function()
    assert.is_true(E.equal(read('[]'),get(scanner().read_module('-- require("no")'),'requires')))
    assert.is_true(E.equal(read('[]'),get(scanner().read_module('local s="require(\\"no\\")'),'requires')))
    local s=scanner().read_module('require ( "a.b" )\nrequire\t\'c\'\nrequire("1bad")\nrequired("no")\nrequire(name)\nrequire\n"newline"\n--[[ require("no")')
    assert.is_true(E.equal(read('["a.b" "c"]'),get(s,'requires')))
  end)

  it('reads requires and public/local function declarations',function()
    local s=scanner().read_module([[local util = require("calc.util")
local cfg = require 'config'
local function between(x) return x end
function M.clamp(x) return between(x) end
M.total = function(xs) return xs end
function Account:deposit(amount) return amount end]])
    assert.is_true(E.equal(read('["calc.util" "config"]'),get(s,'requires')))
    assert.is_true(E.equal(read('[{:name "between" :local true :nested false} {:name "M.clamp" :local false :nested false} {:name "M.total" :local false :nested false} {:name "Account:deposit" :local false :nested false}]'),get(s,'defns')))
  end)
end)
