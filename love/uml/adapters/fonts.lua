-- The viewer's one font: Sarasa Mono SC at two sizes in the imgui atlas.
-- Lookup: explicit path -> UML_CJK_FONT -> provisioned Sarasa -> system YaHei.
-- Messages here are stderr/assertions and stay English.
local S=require('uml.application.strings')
local M={}
M.sizes={body=16,title=20}
M.sarasa='SarasaMonoSC-Regular.ttf'
function M.provisioned(root) return (root or '.'):gsub('\\','/')..'/.uml-viewer/fonts/'..M.sarasa end
local function exists(path) local f=io.open(path,'rb'); if f then f:close(); return true end; return false end
local function warn(text) io.stderr:write(text) end
local warned=false
function M.defaults(root)
  return {env=os.getenv('UML_CJK_FONT'),root=root,windir=os.getenv('WINDIR'),exists=exists,warn=warn}
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
  local yahei=(opts.windir or 'C:/Windows'):gsub('\\','/')..'/Fonts/msyh.ttc'
  if not warned then
    warned=true
    say('uml viewer: Sarasa Mono SC not found at '..sarasa..'; falling back to Microsoft YaHei. Run get-uml-viewer to provision the font.\n')
  end
  return {path=yahei,source='yahei'}
end
-- Adds the body and title fonts to the atlas before imgui.love.BuildFontAtlas.
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
  for _,role in ipairs{'body','title'} do
    local font=atlas:AddFontFromFileTTF(choice.path,M.sizes[role],nil,ranges.Data)
    assert(font~=nil,'CJK font could not be loaded: '..choice.path)
    fonts[role]=font
  end
  io.FontDefault=fonts.body
  M.current=fonts
  return fonts
end
return M
