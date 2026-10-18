local E=require('uml.edn')
local function select(policy,files) return require('uml.domain.language_detection').select(E.parse(policy),files) end
describe('policy language detection',function()
  it('selects each single project marker without scanner IO',function()
    for _,case in ipairs{{'package.json','typescript'},{'Cargo.toml','rust'},{'pyproject.toml','python'},{'src/main.lua','lua'}} do
      assert.is_true(E.equal(E.keyword(case[2]),select('{}',{case[1]})))
    end
  end)
  it('requires explicit language for ambiguous or missing markers',function()
    for _,files in ipairs{{},{'package.json','src/a.lua'},{'Cargo.toml','pyproject.toml'}} do
      local ok,err=pcall(select,'{}',files); assert.is_false(ok); assert.matches(':lang or :sources',err,1,true)
    end
    assert.equals('lua',select('{}',{'a.lua','b.lua'}).name)
  end)
  it('honors explicit language and sources before examining project markers',function()
    assert.equals('python',select('{:lang :python}',{'package.json','a.lua'}).name)
    assert.is_nil(select('{:sources [{:lang :rust :root "src"}]}',{'package.json','a.lua'}))
  end)
  it('routes detected and explicit languages through the IR generation seam',function()
    local IR=require('uml.application.ir_generator'); local G=require('uml.graph')
    G.register('detection-test',function() return E.parse('{:classes [{:id :chosen :name "Chosen" :ns "app.chosen"}] :edges []}') end)
    local doc=IR.document(nil,E.parse('{:lang :detection-test :prefix "app"}'),{'package.json','a.lua'})
    assert.equals('Chosen',E.get(E.get(doc,E.keyword('classes'))[1],E.keyword('name')))
    local ok,err=pcall(IR.document,nil,E.parse('{}'),{'package.json','a.lua'})
    assert.is_false(ok); assert.matches(':lang or :sources',err,1,true)
  end)
  it('generates IR for each detected marker through injected scanner selection',function()
    local IR=require('uml.application.ir_generator'); local G=require('uml.graph')
    for _,case in ipairs{{'package.json','typescript'},{'Cargo.toml','rust'},{'pyproject.toml','python'},{'src/a.lua','lua'}} do
      local previous=G.lookup(case[2]); local written
      G.register(case[2],function() return E.parse('{:classes [{:id :selected :name "Selected" :ns "app.selected"}] :edges []}') end)
      local ok,err=pcall(IR.generate,nil,'policy.edn','out.edn',{
        read=function() return '{:prefix "app"}' end,
        filenames=function() return {case[1]} end,
        write=function(_,text) written=E.parse(text) end
      })
      G.register(case[2],previous)
      assert.is_true(ok,tostring(err))
      assert.equals('Selected',E.get(E.get(written,E.keyword('classes'))[1],E.keyword('name')))
    end
  end)
  it('rejects explicit Clojure including source entries',function()
    for _,policy in ipairs{'{:lang :clojure}','{:sources [{:lang :clojure}]}'} do
      local ok,err=pcall(select,policy,{}); assert.is_false(ok); assert.matches('Clojure is not supported',err,1,true)
    end
  end)
end)
