local Identity=require('uml.domain.identity')
describe('LOVE process identity document',function()
  it('formats both FILETIME halves as exact unsigned decimal without floating point',function()
    assert.are.equal('0',Identity.filetime(0,0))
    assert.are.equal('4294967295',Identity.filetime(0,4294967295))
    assert.are.equal('4294967296',Identity.filetime(1,0))
    assert.are.equal('18446744069414584320',Identity.filetime(4294967295,0))
    assert.are.equal('18446744073709551615',Identity.filetime(4294967295,4294967295))
  end)
  it('rejects values outside the 32-bit FILETIME halves',function()
    for _,pair in ipairs{{-1,0},{4294967296,0},{0,-1},{0,4294967296},{.5,0}} do
      assert.has_error(function() Identity.filetime(pair[1],pair[2]) end)
    end
  end)
  it('converts FILETIME ticks to .NET ticks exactly',function()
    assert.are.equal('504911232000000000',Identity.dotnet_ticks('0'))
    assert.are.equal('504911232000000001',Identity.dotnet_ticks('1'))
    assert.are.equal('18951655305709551615',Identity.dotnet_ticks('18446744073709551615'))
    assert.has_error(function() Identity.dotnet_ticks('abc') end)
  end)
  it('writes a stable JSON record with runtime provenance and an escaped cwd',function()
    local json=Identity.encode{pid=1234,started='18446744073709551615',runtime='love',
      cwd='C:/project A/x "y"\\z'}
    assert.are.equal('{"pid":1234,"started":"18446744073709551615","runtime":"love","cwd":"C:/project A/x \\"y\\"\\\\z"}',json)
  end)
  it('refuses malformed pid, start time and runtime records',function()
    local valid={pid=1,started='4294967296',runtime='love',cwd='C:/p'}
    for _,change in ipairs{
      {pid=0},{pid=-1},{pid=1.5},{started=''},
      {started='4294967296.0'},{started='-1'},{started='abc'},
      {runtime='jvm'},{cwd=''}
    } do
      local record={}; for k,v in pairs(valid) do record[k]=v end
      for k,v in pairs(change) do record[k]=v end
      assert.has_error(function() Identity.encode(record) end)
    end
  end)
end)
