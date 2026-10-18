local Source=require('uml.main.source')

describe('jump to source public contract',function()
  it('requires an explicit supported language without namespace inference',function()
    local files={is_file=function() error('unsupported languages must not access files') end}
    assert.is_nil(Source.member_source({file='example.lua'},files))
    assert.is_nil(Source.member_source({lang='clojure',file='example.clj'},files))
    assert.is_nil(Source.member_source({lang='lua',ns='app.some_module'},files))
  end)
  it('rejects a missing file and never reads it',function()
    assert.is_nil(Source.member_source({lang='lua',file='missing.lua'}, {
      is_file=function() return false end,read=function() error('missing file read') end}))
  end)
  it('preserves literal names, CRLF declaration trimming, and regex word boundaries',function()
    local impl=Source.lookup('typescript')
    local text='export function pickMore() {}\r\nexport function pick$() {}\r\nexport function pick() {}  \r\n'
    assert.equal(2,impl.start_line(text,{name='pick'}))
    assert.equal('export function pick$() {}',impl.extract(text,{name='pick'}))
    assert.is_nil(impl.extract('function pick$() {}',{name='pick$'}))
    assert.equal('function pick$x() {}',impl.extract('function pick$x() {}',{name='pick$'}))
  end)
  it('exposes locate and title without changing namespace spelling',function()
    local impl=Source.lookup('python')
    assert.equal('app.some_module/walk',impl.title({ns='app.some_module',name='walk'}))
    assert.equal('C:/project/src/model.py',impl.locate({file='src\\model.py'}, {root='C:/project',is_file=function() return true end}))
  end)
  it('allows a validated custom source with unknown line and fallback title',function()
    local impl={locate=function() return 'custom.lua' end,extract=function() return 'valid' end,start_line=function() return nil end}
    assert.same({file='custom.lua',title='custom.lua:1',body='whole file',lang='lua'}, Source.member_source({lang='lua',name='pick'},{read=function() return 'whole file' end},impl))
  end)
  it('treats Lua member names literally and prefers declarations over assignments',function()
    local impl=assert(Source.lookup('lua'))
    local text='M.pick = function() end\nfunction Mxpick() end\nfunction M.pick() end\n'
    assert.equal(3,impl.start_line(text,{name='M.pick'}))
  end)
  it('locates the Rust Tauri attribute rather than the function line',function()
    local impl=assert(Source.lookup('rust'))
    local text='pub fn pick() {}\n#[tauri::command]\nfn pick() {}\n'
    assert.same({2,'#[tauri::command]\nfn pick() {}'}, {impl.start_line(text,{name='pick'}),impl.extract(text,{name='pick'})})
  end)
  it('chooses async Python definitions before earlier synchronous definitions',function()
    local impl=assert(Source.lookup('python'))
    assert.equal(2,impl.start_line('def pick(): pass\nasync def pick(): pass\n',{name='pick'}))
  end)
  it('opens an exported TypeScript member with the whole original file',function()
    local body='export function loadBook(): string {\n  return "";\n}\n'
    local result=Source.member_source({lang='typescript',file='f/book.ts',name='loadBook'}, {
      is_file=function(path) return path=='f/book.ts' end,
      read=function() return body end})
    assert.same({file='f/book.ts',title='f/book.ts:1',body=body,line=1,lang='typescript'},result)
  end)
end)
