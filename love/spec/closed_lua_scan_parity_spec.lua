local E=require('uml.edn')
local G=require('uml.domain.geom')
local S=require('uml.lua_language.graph_lua')
local g=G.get
local function by_id(graph,id) for _,c in ipairs(g(graph,'classes')) do if G.name(g(c,'id'))==id then return c end end end
local function names(c) local r={}; for _,op in ipairs(g(c,'ops') or {}) do r[#r+1]=g(op,'name') end; return r end
local function read(path) local f=assert(io.open(path,'rb')); local s=f:read('*a'); f:close(); return s end

describe('closed Lua scanner public output parity',function()
  it('emits public module operations and excludes local functions',function()
    local s=S.scan('C:/project/src',E.parse('{:prefix "app"}'),{
      {path='C:/project/src/app/init.lua',source='local model=require("app.model")\nlocal lfs=require("lfs")\nlocal M={}\nfunction M.run() return model.load() end\nreturn M'},
      {path='C:/project/src/app/model.lua',source='local function hidden() end\nlocal M={}\nfunction M.load() return 1 end\nM.save=function(x) return x end\nreturn M'}
    },'C:/project')
    assert.are.same({'M.run'},names(by_id(s,'app')))
    assert.are.same({'M.load','M.save'},names(by_id(s,'model')))
    assert.is_true(g(by_id(s,'lfs'),'foreign'))
    assert.is_true(E.equal(E.parse('[{:from :app :to :model :kind :dependency} {:from :app :to :lfs :kind :dependency}]'),g(s,'edges')))
  end)
  it('emits the real metrics fixture public operations under joinable namespaces',function()
    local s=S.scan('C:/project/src',E.parse('{:prefix "calc" :ns-prefix ""}'),{
      {path='C:/project/src/calc/init.lua',source=read('lua-fixture/src/calc/init.lua')},
      {path='C:/project/src/calc/util.lua',source=read('lua-fixture/src/calc/util.lua')}
    },'C:/project')
    assert.are.equal('calc',g(by_id(s,'calc'),'ns')); assert.are.equal('calc.util',g(by_id(s,'util'),'ns'))
    assert.are.same({'M.classify','M.total'},names(by_id(s,'calc')))
    assert.are.same({'M.clamp','M.untested'},names(by_id(s,'util')))
  end)
  it('retains an excluded require as foreign with an empty prefix and explicit namespace prefix',function()
    local s=S.scan('C:/project/server',E.parse('{:prefix "" :ns-prefix "server" :exclude ["packages" "_trigger"]}'),{
      {path='C:/project/server/api.lua',source='local vend=require("server.packages.vend")\nfunction M.go() return vend.x end'},
      {path='C:/project/server/packages/vend.lua',source='function M.hidden() end'},
      {path='C:/project/server/_trigger/function_0.lua',source='return {}'},
      {path='C:/project/server/deep/packages/nested.lua',source='return {}'}
    },'C:/project')
    assert.are.equal(2,#g(s,'classes'))
    assert.is_nil(g(by_id(s,'server.api'),'foreign'))
    assert.is_true(g(by_id(s,'server.packages.vend'),'foreign'))
    assert.are.same({},names(by_id(s,'server.packages.vend')))
    assert.is_true(E.equal(E.parse('[{:from :server.api :to :server.packages.vend :kind :dependency}]'),g(s,'edges')))
  end)
end)
