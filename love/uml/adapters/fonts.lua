-- Sarasa Mono SC: body/class text and native raster sizes for canvas corners.
-- Lookup: explicit path -> UML_CJK_FONT -> provisioned Sarasa -> system YaHei.
-- Messages here are stderr/assertions and stay English.
local S=require('uml.application.strings')
local Display=require('uml.domain.display')
local M={}
M.sizes=require('uml.domain.theme').fonts
M.sarasa='SarasaMonoSC-Regular.ttf'
function M.provisioned(root) return (root or '.'):gsub('\\','/')..'/.uml-viewer/fonts/'..M.sarasa end
local function exists(path) local f=io.open(path,'rb'); if f then f:close(); return true end; return false end
local function warn(text) io.stderr:write(text) end
local warned=false
-- System CJK fallbacks, in preference order, for a platform with no
-- provisioned Sarasa. Windows uses Microsoft YaHei; POSIX uses the Noto CJK
-- families shipped by Ubuntu/macOS package sets.
local SYSTEM_FONTS={
  yahei={'C:/Windows/Fonts/msyh.ttc','C:/Windows/Fonts/msyh.ttf'},
  noto={'/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc',
        '/usr/share/fonts/opentype/noto/NotoSansCJKsc-Regular.otf',
        '/System/Library/Fonts/PingFang.ttc'},
}
function M.defaults(root)
  local windir=os.getenv('WINDIR')
  local candidates={}
  if windir and windir~='' then
    candidates[#candidates+1]=(windir:gsub('\\','/')..'/Fonts/msyh.ttc')
    candidates[#candidates+1]=(windir:gsub('\\','/')..'/Fonts/msyh.ttf')
  end
  for _,path in ipairs(SYSTEM_FONTS.noto) do candidates[#candidates+1]=path end
  return {env=os.getenv('UML_CJK_FONT'),root=root,windir=windir,candidates=candidates,
    exists=exists,warn=warn}
end
function M.resolve(opts)
  local has=opts.exists or exists; local say=opts.warn or warn
  local env=opts.env~='' and opts.env or nil
  for _,c in ipairs{{opts.explicit,'explicit','font path'},{env,'env','UML_CJK_FONT'}} do
    if c[1] then
      if has(c[1]) then return {path=c[1],source=c[2]} end
      say('uml viewer: '..c[3]..' not found: '..c[1]..'\n')
    end
  end
  local sarasa=M.provisioned(opts.root)
  if has(sarasa) then return {path=sarasa,source='sarasa'} end
  local fallbacks=opts.candidates
  if not fallbacks then
    fallbacks={}
    local windir=opts.windir or 'C:/Windows'
    fallbacks[1]=(windir:gsub('\\','/'))..'/Fonts/msyh.ttc'
    for _,path in ipairs(SYSTEM_FONTS.noto) do fallbacks[#fallbacks+1]=path end
  end
  for _,path in ipairs(fallbacks) do
    if has(path) then
      if not warned then
        warned=true
        say('uml viewer: Sarasa Mono SC not found at '..sarasa..'; falling back to '..path
          ..'. Run get-uml-viewer to provision the font.\n')
      end
      return {path=path,source=path:find('msyh',1,true) and 'yahei' or 'system'}
    end
  end
  local yahei=fallbacks[1]
  if not warned then
    warned=true
    say('uml viewer: Sarasa Mono SC not found at '..sarasa..'; falling back to Microsoft YaHei. Run get-uml-viewer to provision the font.\n')
  end
  return {path=yahei,source='yahei'}
end
-- Adds each actually used raster size before imgui.love.BuildFontAtlas.
-- The merged glyph ranges are kept on the returned table so the vector stays
-- alive through the atlas build.
function M.load(imgui,opts)
  local choice=M.resolve(opts or {})
  local io=imgui.GetIO(); local atlas=io.Fonts
  local builder=imgui.ImFontGlyphRangesBuilder()
  builder:AddRanges(atlas:GetGlyphRangesDefault())
  builder:AddRanges(atlas:GetGlyphRangesGreek())
  builder:AddRanges(atlas:GetGlyphRangesChineseSimplifiedCommon())
  builder:AddText(S.glyph_text(),nil)
  local ranges=builder:BuildRanges()
  local fonts={path=choice.path,source=choice.source,ranges=ranges,builder=builder}
  -- opts.display (domain.display) rasterises at device px; widget text then
  -- measures size*ui window px through imgui's global font scale.
  local display=opts and opts.display
  local raster=function(size) return display and Display.font_raster(display,size) or size end
  if display then io.FontGlobalScale=Display.font_global_scale(display) end
  for _,role in ipairs{'badge','level','marker','code','body'} do
    local font=atlas:AddFontFromFileTTF(choice.path,raster(M.sizes[role]),nil,ranges.Data)
    assert(font~=nil,'CJK font could not be loaded: '..choice.path)
    fonts[role]=font
  end
  fonts.title=fonts.body
  io.FontDefault=fonts.body
  M.current=fonts
  return fonts
end
return M
