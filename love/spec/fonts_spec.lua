local function fresh() package.loaded['uml.adapters.fonts']=nil; return require('uml.adapters.fonts') end
local function chain(files,env)
  local warnings={}
  local opts={explicit=env.explicit,env=env.env,root='C:/proj',windir='C:/Windows',
    exists=function(p) return files[p]==true end,warn=function(s) warnings[#warnings+1]=s end}
  return opts,warnings
end
local sarasa='C:/proj/.uml-viewer/fonts/SarasaMonoSC-Regular.ttf'
local yahei='C:/Windows/Fonts/msyh.ttc'
describe('viewer font lookup chain',function()
  it('prefers an explicit path, then UML_CJK_FONT, then provisioned Sarasa, then YaHei',function()
    local F=fresh()
    local all={['C:/x.ttf']=true,['C:/env.ttf']=true,[sarasa]=true,[yahei]=true}
    local opts,warnings=chain(all,{explicit='C:/x.ttf',env='C:/env.ttf'})
    assert.are.same({path='C:/x.ttf',source='explicit'},F.resolve(opts))
    opts=chain(all,{env='C:/env.ttf'}); assert.are.same({path='C:/env.ttf',source='env'},F.resolve(opts))
    opts=chain(all,{}); assert.are.same({path=sarasa,source='sarasa'},F.resolve(opts))
    assert.are.equal(0,#warnings)
  end)
  it('falls back to YaHei with one stderr warning that names get-uml-viewer',function()
    local F=fresh()
    local opts,warnings=chain({[yahei]=true},{})
    assert.are.same({path=yahei,source='yahei'},F.resolve(opts))
    F.resolve(opts)
    assert.are.equal(1,#warnings)
    assert.is_truthy(warnings[1]:find('get-uml-viewer',1,true)); assert.is_truthy(warnings[1]:find('Sarasa',1,true))
  end)
  it('reports a missing override and continues down the chain',function()
    local F=fresh()
    local opts,warnings=chain({[sarasa]=true},{env='C:/gone.ttf'})
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
    assert.are.same({16,20},{added[1].size,added[2].size})
    for _,f in ipairs(added) do assert.are.equal(sarasa,f.path); assert.are.equal('merged',f.ranges) end
    assert.are.equal(added[1],fonts.body); assert.are.equal(added[2],fonts.title); assert.are.equal(added[1],io.FontDefault)
    assert.are.equal('sarasa',fonts.source); assert.are.equal(sarasa,fonts.path)
    assert.are.equal(16,F.sizes.body); assert.are.equal(20,F.sizes.title)
  end)
  it('keeps the untranslated assertion when no font loads',function()
    local F=fresh()
    local atlas={GetGlyphRangesDefault=function() end,GetGlyphRangesGreek=function() end,GetGlyphRangesChineseSimplifiedCommon=function() end,AddFontFromFileTTF=function() return nil end}
    local imgui={GetIO=function() return {Fonts=atlas} end,ImFontGlyphRangesBuilder=function() return {AddRanges=function() end,AddText=function() end,BuildRanges=function() return {} end} end}
    local opts=chain({},{})
    assert.has_error(function() F.load(imgui,opts) end,'CJK font could not be loaded: '..yahei)
  end)
end)
