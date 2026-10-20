local V=require('uml.application.source_view')
describe('source presentation',function()
  it('preserves source text while coloring keywords, strings and comments for supported scanners',function()
    for _,case in ipairs{{'lua','local x = "value" -- note','local','"value"','-- note'},
      {'python','return "value" # note','return','"value"','# note'},
      {'typescript','return "value" // note','return','"value"','// note'},
      {'rust','let x = "value"; // note','let','"value"','// note'}} do
      local runs=V.tokens(case[2],case[1]); local joined,by='',{}
      for _,r in ipairs(runs) do joined=joined..r.text; by[r.text]=r.kind end
      assert.equal(case[2],joined)
      assert.equal('keyword',by[case[3]]); assert.equal('string',by[case[4]]); assert.equal('comment',by[case[5]])
    end
  end)
  it('keeps Chinese identifiers in valid UTF8 token runs',function()
    local runs=V.tokens('local 变量 = 1','lua'); local found=false
    for _,run in ipairs(runs) do
      -- Common Lua5.1/5.4 check: no run starts/ends inside a UTF8 codepoint.
      local i=1
      while i<=#run.text do
        local b=run.text:byte(i); local n=b<128 and 1 or b>=194 and b<224 and 2 or b<240 and 3 or 4
        assert.is_true(b<128 or b>=194)
        for j=1,n-1 do local c=run.text:byte(i+j); assert.is_true(c~=nil and c>=128 and c<192) end
        i=i+n
      end
      if run.text=='变量' then found=true end
    end
    assert.is_true(found)
  end)
  it('maps tab-expanded drag boundaries back to exact source bytes in either direction',function()
    local display=V.display('\tfunction 变量()')
    assert.equal('  function 变量()',display.text)
    local rows={{text='\tfunction 变量()'},{text='\treturn 变量'}}
    local a={row=1,col=display.to_source[3]}; local b={row=1,col=display.to_source[11]}
    assert.equal('function',V.selected(rows,b,a))
    local second=V.display(rows[2].text)
    assert.equal('function 变量()\n\treturn',V.selected(rows,{row=2,col=second.to_source[9]},a))
    assert.equal(3,display.to_display[2])
  end)
  it('copies a backwards selection across rows without including the gutter',function()
    local rows={{text='local 中文 = 1'},{text='return 中文'}}
    assert.equal('中文 = 1\nreturn',V.selected(rows,{row=2,col=7},{row=1,col=7}))
    assert.equal('中文',V.selected(rows,{row=1,col=7},{row=1,col=13}))
  end)
end)
