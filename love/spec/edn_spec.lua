local edn = require('uml.edn')

describe('EDN protocol', function()
  it('preserves scalar distinctions, nil positions, comments and vector keys', function()
    local value = edn.parse('; queue\n{:queue [nil false true "line\\n中文" -1.25e2] :edge-kinds {[:a :b] :association} :levels [domain engine]}')
    local queue = edn.get(value, edn.keyword('queue'))
    assert.are.equal(edn.nil_value, queue[1])
    assert.is_false(queue[2])
    assert.is_true(queue[3])
    assert.are.equal('line\n中文', queue[4])
    assert.are.equal(-125, queue[5])
    assert.is_true(edn.equal(edn.keyword('association'), edn.get(edn.get(value, edn.keyword('edge-kinds')), edn.vector({edn.keyword('a'), edn.keyword('b')}))))
    assert.is_true(edn.equal(edn.symbol('domain'), edn.get(value, edn.keyword('levels'))[1]))
    assert.is_true(edn.equal(value, edn.parse(edn.encode(value))))
    assert.is_false(edn.equal(edn.keyword('a'), 'a'))
  end)
  it('preserves inf and nan spellings as symbols across parse and encode',function()
    for _,token in ipairs({'inf','nan','Inf','NaN','+inf','-inf','+nan','-nan','infinity'}) do
      local value=edn.parse(token)
      assert.is_true(edn.equal(edn.symbol(token),value),token)
      assert.are.equal(token,edn.encode(value))
    end
  end)
  it('accepts decimal EDN numbers and rejects Lua numeric extensions',function()
    local examples={{'0',0},{'+42',42},{'-42',-42},{'1.',1},{'0.125',0.125},{'-1.25e2',-125},{'1E+3',1000},{'1e-3',0.001},{'1234567890123.5',1234567890123.5},{'1.2345678901235e12',1234567890123.5},{'1e-100',1e-100},{'00042',42}}
    for _,example in ipairs(examples) do
      local value=edn.parse(example[1])
      assert.are.equal(example[2],value,example[1])
      assert.are.equal(value,edn.parse(edn.encode(value)))
    end
    for _,token in ipairs({'0x1p2','0xff','1e','1e+','1.2.3','1N','1M','1/2'}) do
      assert.has_error(function() edn.parse(token) end,nil,token)
    end
  end)
  it('rejects non-finite numbers at parse and encode boundaries',function()
    for _,token in ipairs({'1e999','-1e999'}) do
      local ok,message=pcall(edn.parse,token)
      assert.is_false(ok,token)
      assert.matches('non%-finite EDN number',message)
    end
    for _,value in ipairs({math.huge,-math.huge,0/0}) do
      assert.has_error(function() edn.encode(value) end,'non-finite EDN number')
    end
  end)
  it('rejects unsafe integers rather than rounding them during parse',function()
    for _,token in ipairs({'9007199254740992','9007199254740993','9223372036854775807','-9007199254740993','-9223372036854775808','9.007199254740993e15'}) do
      local ok,message=pcall(edn.parse,token)
      assert.is_false(ok,token)
      assert.matches('outside supported exact integer range',message)
    end
    for _,token in ipairs({'9007199254740991','-9007199254740991','0'}) do
      assert.are.equal(token,edn.encode(edn.parse(token)))
    end
  end)
  it('rejects direct unsafe Lua numbers before encode can corrupt them',function()
    for _,value in ipairs({9007199254740992,9007199254740993,9223372036854775807,-9007199254740993}) do
      local ok,message=pcall(edn.encode,value)
      assert.is_false(ok)
      assert.matches('outside supported exact integer range',message)
    end
    assert.are.equal('9007199254740991',edn.encode(9007199254740991))
    assert.are.equal('-9007199254740991',edn.encode(-9007199254740991))
  end)
  it('reads Unicode escapes and preserves supplementary characters',function()
    assert.are.equal('中文😀',edn.parse('"\\u4e2d\\u6587\\ud83d\\ude00"'))
    assert.are.equal('\0\1\n',edn.parse(edn.encode('\0\1\n')))
  end)
  it('rejects malformed input instead of returning a partial document',function()
    for _,text in ipairs({'[1','{:a}','{:a 1 :a 2}','true false','"\\q"','#inst "x"'}) do
      assert.has_error(function() edn.parse(text) end)
    end
  end)
  it('compares maps independent of order and retrieves false and nil keys',function()
    assert.is_true(edn.equal(edn.parse('{:a 1 :b 2}'),edn.parse('{:b 2 :a 1}')))
    local map=edn.parse('{false nil nil false [nil] 42}')
    assert.are.equal(edn.nil_value,edn.get(map,false))
    assert.is_false(edn.get(map,edn.nil_value))
    assert.are.equal(42,edn.get(map,edn.vector({edn.nil_value})))
  end)
end)
