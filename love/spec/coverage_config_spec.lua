describe('whole business tree coverage configuration',function()
  it('includes untested adapters and maps module and slash aliases to one source',function()
    local c=assert(loadfile('scripts/coverage_config.lua'))()
    assert.equal('love/uml/application/detail.lua',c.modules['uml.application.detail'])
    assert.equal(c.modules['uml.application.detail'],c.modules['love/uml/application/detail'])
    local found=false
    for _,path in ipairs(c.includeuntestedfiles) do
      assert.truthy(path:match('^love/uml/'))
      if path=='love/uml/adapters/viewer.lua' then found=true end
    end
    assert.is_true(found,'untested native adapter must stay in whole-tree coverage')
    assert.is_true(#c.includeuntestedfiles>50)
  end)
end)
