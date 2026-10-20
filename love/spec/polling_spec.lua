local Poll=require('uml.domain.polling')
local E=require('uml.edn')
local G=require('uml.domain.geom')
local Mailbox=require('uml.adapters.mailbox')
describe('mailbox polling cache',function()
  it('parses only changed empty mailboxes and detects missing, empty and new files',function()
    local stamp,text,reads,parses=nil,nil,0,0
    local box=Mailbox.open('injected',{mtime=function() return stamp end,
      read=function() reads=reads+1; return text end,
      parse=function(s) parses=parses+1; return E.parse(s) end})
    assert.is_nil(box.take()); assert.is_nil(box.take()); assert.are.equal(1,reads)
    stamp=1; text='{:next-id 1 :queue []}'
    assert.is_nil(box.take()); assert.is_nil(box.take()); assert.are.equal(2,reads); assert.are.equal(1,parses)
    stamp=2; text=''; assert.is_nil(box.take()); assert.are.equal(2,parses)
    stamp=nil; text=nil; assert.is_nil(box.take()); assert.are.equal(4,reads)
  end)
  it('does not reparse unchanged commands already behind the restored cursor',function()
    local parses=0
    local box=Mailbox.open('injected',{mtime=function() return 1 end,
      read=function() return '{:next-id 2 :queue [{:id 1 :op :context}]}' end,
      parse=function(text) parses=parses+1; return E.parse(text) end})
    assert.is_nil(box.take(1)); assert.is_nil(box.take(1)); assert.are.equal(1,parses)
    assert.are.equal(1,G.get(box.take(0),'id'))
  end)
  it('does no filesystem work on frames before the deadline',function()
    local now,reads=0,0
    local poll=Poll.new(function() return now end)
    local box=Mailbox.open('injected',{mtime=function() reads=reads+1; return 1 end,
      read=function() return '{:next-id 1 :queue []}' end})
    for _,time in ipairs{0,.050,.100,.249,.250,.300,.499,.500} do
      now=time; if poll.due() then box.take() end
    end
    assert.are.equal(3,reads)
  end)
end)
describe('mailbox destructive drain',function()
  it('rereads each pop and preserves appended mail even when timestamps repeat',function()
    local text='{:next-id 3 :queue [{:id 1 :op :context} {:id 2 :op :display}]}'
    local box=Mailbox.open('injected',{mtime=function() return 1 end,
      read=function() return text end,write=function(_,value) text=value end})
    assert.are.equal(1,G.get(box.take(),'id'))
    text='{:next-id 4 :queue [{:id 2 :op :display} {:id 3 :op :quit-for-restart}]}'
    assert.are.equal(2,G.get(box.take(),'id'))
    assert.are.equal(3,G.get(box.take(),'id')); assert.is_nil(box.take())
    assert.are.equal(4,G.get(E.parse(text),'next-id'))
  end)
end)
describe('document polling',function()
  it('checks immediately and at 250ms boundaries, with an explicit refresh bypass',function()
    local now=0
    local poll=Poll.new(function() return now end)
    assert.is_true(poll.due())
    now=.249; assert.is_false(poll.due())
    now=.250; assert.is_true(poll.due())
    now=.300; assert.is_true(poll.due(true))
    now=.549; assert.is_false(poll.due())
    now=.550; assert.is_true(poll.due())
  end)
end)
