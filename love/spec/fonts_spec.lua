local function fresh() package.loaded['uml.adapters.fonts']=nil; return require('uml.adapters.fonts') end
local function chain(files,env)
  local warnings={}
  local opts={explicit=env.explicit,env=env.env,root='/proj',
    exists=function(p) return files[p]==true end,warn=function(s) warnings[#warnings+1]=s end}
  return opts,warnings
end
local sarasa='/proj/.uml-viewer/fonts/SarasaMonoSC-Regular.ttf'
local system='/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc'
describe('viewer font lookup chain',function()
  it('prefers an explicit path, then UML_CJK_FONT, then provisioned Sarasa, then a system CJK font',function()
    local F=fresh()
    local all={['/x.ttf']=true,['/env.ttf']=true,[sarasa]=true,[system]=true}
    local opts,warnings=chain(all,{explicit='/x.ttf',env='/env.ttf'})
    assert.are.same({path='/x.ttf',source='explicit'},F.resolve(opts))
    opts=chain(all,{env='/env.ttf'}); assert.are.same({path='/env.ttf',source='env'},F.resolve(opts))
    opts=chain(all,{}); assert.are.same({path=sarasa,source='sarasa'},F.resolve(opts))
    assert.are.equal(0,#warnings)
  end)
  it('falls back to a system CJK font with one stderr warning that names get-uml-viewer',function()
    local F=fresh()
    local opts,warnings=chain({[system]=true},{})
    assert.are.same({path=system,source='system'},F.resolve(opts))
    F.resolve(opts)
    assert.are.equal(1,#warnings)
    assert.is_truthy(warnings[1]:find('get-uml-viewer',1,true)); assert.is_truthy(warnings[1]:find('Sarasa',1,true))
  end)
  it('falls back only to the Noto CJK fonts Ubuntu ships',function()
    local F=fresh()
    assert.are.same({'/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc',
      '/usr/share/fonts/opentype/noto/NotoSansCJKsc-Regular.otf'},F.defaults('/proj').candidates)
  end)
  it('reports a missing override and continues down the chain',function()
    local F=fresh()
    local opts,warnings=chain({[sarasa]=true},{env='/gone.ttf'})
    assert.are.same({path=sarasa,source='sarasa'},F.resolve(opts))
    assert.are.equal(1,#warnings); assert.is_truthy(warnings[1]:find('UML_CJK_FONT',1,true))
  end)
  it('builds one atlas font per size with common Chinese, Greek and every string-table glyph',function()
    local F=fresh(); local S=require('uml.application.strings')
    local added,texts,built={},{},false
    local atlas={GetGlyphRangesDefault=function() return 'default' end,GetGlyphRangesGreek=function() return 'greek' end,
      GetGlyphRangesChineseSimplifiedCommon=function() return 'chinese' end,
      AddFontFromFileTTF=function(_,path,size,cfg,ranges) local f={path=path,size=size,ranges=ranges}; added[#added+1]=f; return f end}
    local io={Fonts=atlas}
    local ranges_seen={}
    local imgui={GetIO=function() return io end,ImFontGlyphRangesBuilder=function()
      return {AddRanges=function(_,r) ranges_seen[#ranges_seen+1]=r end,AddText=function(_,t) texts[#texts+1]=t end,
        BuildRanges=function() built=true; return {Data='merged'} end} end}
    local opts=chain({[sarasa]=true},{})
    local fonts=F.load(imgui,opts)
    assert.are.same({'default','greek','chinese'},ranges_seen)
    assert.are.equal(S.glyph_text(),texts[1])
    assert.is_true(built)
    local sizes={}; for _,font in ipairs(added) do sizes[#sizes+1]=font.size end
    assert.are.same({9,11,12,13,18},sizes)
    for _,f in ipairs(added) do assert.are.equal(sarasa,f.path); assert.are.equal('merged',f.ranges) end
    assert.are.equal(added[4],fonts.code); assert.are.equal(13,F.sizes.code)
    assert.are.equal(added[5],fonts.body); assert.are.equal(fonts.body,fonts.title); assert.are.equal(fonts.body,io.FontDefault)
    assert.are.equal(added[1],fonts.badge); assert.are.equal(added[2],fonts.level); assert.are.equal(added[3],fonts.marker)
    assert.are.equal('sarasa',fonts.source); assert.are.equal(sarasa,fonts.path)
    assert.are.equal(18,F.sizes.body); assert.are.equal(18,F.sizes.title)
  end)
  it('rasterises each size at the display DPI and undoes only the backend framebuffer scale',function()
    local F=fresh(); local Display=require('uml.domain.display')
    local added={}
    local atlas={GetGlyphRangesDefault=function() end,GetGlyphRangesGreek=function() end,GetGlyphRangesChineseSimplifiedCommon=function() end,
      AddFontFromFileTTF=function(_,path,size) local f={size=size}; added[#added+1]=f; return f end}
    local io={Fonts=atlas}
    local imgui={GetIO=function() return io end,ImFontGlyphRangesBuilder=function()
      return {AddRanges=function() end,AddText=function() end,BuildRanges=function() return {Data='merged'} end} end}
    local opts=chain({[sarasa]=true},{}); opts.display=Display.new{window_w=1500,window_h=920,os_scale=1.5}
    F.load(imgui,opts)
    local sizes={}; for _,font in ipairs(added) do sizes[#sizes+1]=font.size end
    assert.are.same({13.5,16.5,18,19.5,27},sizes); assert.are.equal(1,io.FontGlobalScale)
    added={}; opts.display=Display.new{window_w=1500,window_h=920,os_scale=2,framebuffer_scale=2}
    F.load(imgui,opts)
    assert.are.equal(36,added[5].size); assert.are.equal(.5,io.FontGlobalScale)
  end)
  it('keeps the untranslated assertion when no font loads',function()
    local F=fresh()
    local atlas={GetGlyphRangesDefault=function() end,GetGlyphRangesGreek=function() end,GetGlyphRangesChineseSimplifiedCommon=function() end,AddFontFromFileTTF=function() return nil end}
    local imgui={GetIO=function() return {Fonts=atlas} end,ImFontGlyphRangesBuilder=function() return {AddRanges=function() end,AddText=function() end,BuildRanges=function() return {} end} end}
    local opts=chain({},{})
    assert.has_error(function() F.load(imgui,opts) end,'CJK font could not be loaded: '..system)
  end)
end)
