local L=require('uml.engine.layout')
describe('layout text width estimate',function()
  it('keeps the JVM width for ASCII, two-byte and astral text',function()
    assert.are.equal(8,L.char_w)
    assert.are.equal(0,L.text_w(nil))
    assert.are.equal(40,L.text_w('Hello'))
    assert.are.equal(24,L.text_w('«α»'))
    -- U+1F600 is one UTF-16 surrogate pair on the JVM: two units.
    assert.are.equal(16,L.text_w('\240\159\152\128'))
  end)
  it('counts full-width CJK characters as two English characters',function()
    assert.are.equal(32,L.text_w('中文'))
    assert.are.equal(16+8*3+16,L.text_w('类Box提'))
    assert.are.equal(16,L.text_w('，'))   -- U+FF0C full-width comma
    assert.are.equal(16,L.text_w('。'))   -- U+3002 CJK punctuation
    assert.are.equal(16,L.text_w('한'))   -- Hangul syllable
    assert.are.equal(8,L.text_w('—'))     -- U+2014 stays half width
    assert.are.equal(8,L.text_w('ｱ'))     -- U+FF71 half-width katakana
  end)
  it('widens class boxes that hold Chinese names',function()
    local E=require('uml.edn')
    local w=L.class_box_size(E.parse('{:name "中文类名中文类名中文类名"}'))
    assert.are.equal(24+12*16,w)
  end)
end)
