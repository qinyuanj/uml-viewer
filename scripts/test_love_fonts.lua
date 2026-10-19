-- Runs only in a newly launched native viewer. Records which font the lookup
-- chain chose and checks every string-table glyph at every used raster size.
local S=require('uml.application.strings')
local function codepoints(s)
  local out,seen={},{}
  for ch in s:gmatch('[%z\1-\127\194-\244][\128-\191]*') do
    local a,b,c,d=ch:byte(1,-1); local code
    if #ch==1 then code=a elseif #ch==2 then code=(a-192)*64+(b-128)
    elseif #ch==3 then code=(a-224)*4096+(b-128)*64+(c-128) else code=(a-240)*262144+(b-128)*4096+(c-128)*64+(d-128) end
    if code>32 and not seen[code] then seen[code]=true; out[#out+1]=code end
  end
  return out
end
return function(frame,state,ui)
  if frame~=3 then return end
  local fonts=assert(ui.fonts,'viewer did not expose loaded fonts')
  local expect=os.getenv('UML_FONT_TEST_EXPECT')
  assert(fonts.source==expect,'font source '..tostring(fonts.source)..' expected '..tostring(expect))
  local missing={}
  local probe=S.glyph_text()..(os.getenv('UML_FONT_TEST_EXTRA') or '')
  local width,height=love.graphics.getDimensions()
  assert(width==1500 and height==920,'default viewer window must be 1500x920')
  for _,role in ipairs{'body','title','badge','level','marker'} do
    local font=assert(fonts[role],'missing '..role..' font')
    assert(math.abs(font.FontSize-({body=18,title=18,badge=9,level=11,marker=12})[role])<0.01,role..' font size '..tostring(font.FontSize))
    for _,code in ipairs(codepoints(probe)) do
      if font:FindGlyphNoFallback(code)==nil then missing[#missing+1]=role..':'..string.format('U+%04X',code) end
    end
  end
  assert(#missing==0,'missing glyphs '..table.concat(missing,' '))
  local f=assert(io.open(os.getenv('UML_VIEWER_EVIDENCE')..'/fonts-pass.txt','wb'))
  f:write('PASS '..fonts.source..' '..fonts.path..' body=18 title=18 badge=9 level=11 marker=12 glyphs='..#codepoints(probe)..'\n'); f:close()
  love.event.quit(0)
end
