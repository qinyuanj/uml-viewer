local E=require('uml.edn')
local G=require('uml.domain.geom')
local IR=require('uml.domain.ir')
local D=require('uml.application.document')
local B=require('uml.adapters.class_boxes')
local function scene(classes) return D.compile_view(IR.read_document('{:packages [{:label "P" :classes '..classes..'}]}')) end
local function select_commands(s,kind)
  local xs={}; for _,c in ipairs(B.commands(s)) do if c.kind==kind then xs[#xs+1]=c end end; return xs
end
describe('class box public renderer commands',function()
  it('shows separate C and M status dots, keeping either absent metric red',function()
    local s=scene('[{:id :a :crap {:mu 2} :killed 9 :survived 1} {:id :b :killed 1 :survived 0} {:id :c :crap {:mu 2}} {:id :d}]')
    local dots=select_commands(s,'quality')
    local statuses={}; for _,d in ipairs(dots) do if d.owner=='class' then statuses[#statuses+1]={d.text,d.band}; assert.is_true(d.x-d.radius>=G.get(d.rect,'x')); assert.is_true(d.y-d.radius>=G.get(d.rect,'y')); assert.is_true(d.x+d.radius<=G.right(d.rect)); assert.is_true(d.y+d.radius<=G.bottom(d.rect)) end end
    assert.are.same({{'C','green'},{'M','yellow'},{'C','red'},{'M','green'},{'C','green'},{'M','red'},{'C','red'},{'M','red'}},statuses)
  end)
  it('shows white abstract/interface markers and level zero inside existing bounds',function()
    local s=scene('[{:id :a :stereotype :abstract :level 0} {:id :b :stereotype :interface}]')
    local markers=select_commands(s,'marker'); local levels=select_commands(s,'level')
    assert.are.equal(2,#markers); assert.are.equal('α',markers[1].text); assert.are.equal('I',markers[2].text)
    for _,c in ipairs(markers) do assert.are.equal(0xffffffff,c.color); assert.is_true(c.x>=G.get(c.rect,'x')); assert.is_true(c.x+8<G.right(c.rect)); assert.is_true(c.y>=G.get(c.rect,'y')) end
    assert.are.equal('0',levels[1] and levels[1].text); assert.are.equal(G.get(levels[1].rect,'x')+4,levels[1].x)
  end)
  it('distinguishes italic classifier and grouping names from plain class and member text',function()
    local s=scene('[{:id :a :stereotype :interface} {:id :b :stereotype :abstract} {:id :c :drill? true} {:id :d :contents [{:id :kid :name "Kid"}]} {:id :e :stereotype :class :fields ["field"]}]')
    local classes=G.get(s,'classes'); G.put(classes[3],'drill?',true); G.put(classes[4],'contents',E.parse('[{:id :kid :name "Kid"}]'))
    local styles={}; for _,c in ipairs(select_commands(s,'text')) do if c.role=='name' then styles[#styles+1]=c.italic; assert.are.equal(18,c.size); assert.is_true(c.bold) elseif c.text=='field' then assert.is_false(c.italic); assert.is_false(c.bold); assert.are.equal(18,c.size) end end
    assert.are.same({true,true,true,true,false},styles)
  end)
  it('renders italic names as slanted glyph geometry while leaving members upright',function()
    local vertices={Size=0,Data={}}; local painted={}
    local list={VtxBuffer=vertices,AddRect=function() end,AddLine=function() end,AddCircleFilled=function() end,
      AddText_FontPtr=function(_,font,size,point,color,text)
        local i=vertices.Size; vertices.Data[i]={pos={x=point.x,y=point.y}}; vertices.Data[i+1]={pos={x=point.x,y=point.y+size}}; vertices.Size=i+2
        painted[text]={vertices.Data[i].pos,vertices.Data[i+1].pos}
      end}
    B.new({ImVec2_Float=function(x,y) return {x=x,y=y} end,GetBackgroundDrawList_Nil=function() return list end}).draw(scene('[{:id :a :name "Interface" :stereotype :interface :fields ["member"]}]'),1000,1000)
    assert.is_true(painted.Interface[1].x>painted.Interface[2].x); assert.are.equal(painted.member[1].x,painted.member[2].x)
  end)
  it('wires pooled package metrics into separate visible status dots and stroke',function()
    local s=scene('[{:id :a :crap {:mu 2 :sigma 0 :n 3} :killed 9 :survived 1 :ops ["a" "b" "c"]} {:id :b :crap {:mu 20 :sigma 0 :n 1} :killed 1 :survived 1}]')
    local p=G.get(s,'packages')[1]
    assert.are.equal('P',G.get(p,'title')); assert.is_true(E.equal(E.parse('{:mu 6.5 :max 20 :sigma 7.8 :n 4}'),G.get(p,'crap'))); assert.are.equal(10,G.get(p,'killed')); assert.are.equal(2,G.get(p,'survived'))
    local package=select_commands(s,'package')[1]; assert.are.equal('red',package.band); assert.are.equal('P',package.text)
    local statuses={}; for _,c in ipairs(select_commands(s,'quality')) do if c.owner=='package' then statuses[#statuses+1]={c.text,c.band} end end
    assert.are.same({{'C','red'},{'M','yellow'}},statuses)
    local absent=scene('[{:id :a :crap 2 :killed 1 :survived 0} {:id :b :ops ["x" "y"]}]')
    assert.are.equal(2,G.get(G.get(absent,'packages')[1],'mut-gap'))
    assert.are.equal('red',select_commands(absent,'package')[1].band)
  end)
  it('draws the abstract alpha as a font glyph now that Greek is in the atlas',function()
    local texts={}; local list={VtxBuffer={Size=0,Data={}},AddRect=function() end,AddCircleFilled=function() end,AddLine=function() end,
      AddText_FontPtr=function(_,font,size,point,color,text) texts[#texts+1]={font=font,text=text,color=color} end}
    local fonts={body={name='body'},title={name='title'}}
    B.new({ImVec2_Float=function(x,y) return {x=x,y=y} end,GetBackgroundDrawList_Nil=function() return list end},fonts).draw(scene('[{:id :a :stereotype :abstract}]'),1000,1000)
    local alpha; for _,t in ipairs(texts) do if t.text=='α' then alpha=t end end
    assert.is_not_nil(alpha); assert.are.equal(fonts.body,alpha.font); assert.are.equal(0xffffffff,alpha.color)
  end)
  it('draws names in the 18px title font and members in the 18px body font, fitted inside the box',function()
    -- A monospace fake: half-width glyphs advance 0.5em, CJK 1em (Sarasa Mono).
    local L=require('uml.engine.layout')
    local function font(name) return {name=name,CalcTextSizeA=function(_,size,_,_,text) return {x=L.text_w(text)/16*size} end} end
    local fonts={body=font('body'),title=font('title')}
    local drawn={}
    local list={VtxBuffer={Size=0,Data={}},AddRect=function() end,AddCircleFilled=function() end,AddLine=function() end,
      AddText_FontPtr=function(_,f,size,point,color,text) drawn[#drawn+1]={font=f,size=size,x=point.x,y=point.y,text=text} end}
    local s=scene('[{:id :a :name "AVeryLongClassNameThatFillsTheBox" :fields ["field"]} {:id :b :name "短"} {:id :c :name "中文类名中文类名中文类名"}]')
    B.new({ImVec2_Float=function(x,y) return {x=x,y=y} end,GetBackgroundDrawList_Nil=function() return list end},fonts).draw(s,1000,1000,{zoom=1,cam_x=0,cam_y=0})
    local rects={}; for _,c in ipairs(G.get(s,'classes')) do rects[G.get(c,'name')]=G.get(c,'rect') end
    local function passes(text) local t={}; for _,d in ipairs(drawn) do if d.text==text then t[#t+1]=d end end; return t end
    for name,r in pairs(rects) do
      local p=passes(name); assert.are.equal(2,#p,'bold name drawn twice: '..name)
      assert.are.equal(fonts.title,p[1].font); assert.is_true(p[2].x>p[1].x)
      local right=p[2].x+fonts.title:CalcTextSizeA(p[2].size,1e9,0,name).x
      assert.is_true(right<=G.right(r)-12+1e-9,'name overflows: '..name)
      assert.is_true(p[1].size<=18)
    end
    assert.are.equal(18,passes('短')[1].size)
    assert.is_true(passes('AVeryLongClassNameThatFillsTheBox')[1].size<18)
    local field=passes('field')[1]; assert.are.equal(fonts.body,field.font); assert.are.equal(18,field.size)
  end)
  it('keeps 18px names clear of the member rule, the stereotype line and the corner badges',function()
    local L=require('uml.engine.layout')
    local font={CalcTextSizeA=function(_,size,_,_,text) return {x=L.text_w(text)/16*size} end}
    local texts,rules,dots={},{},{}
    local list={VtxBuffer={Size=0,Data={}},AddRect=function() end,
      AddLine=function(_,a,b,color) if color==0xff777777 then rules[#rules+1]={left=a.x,right=b.x,y=a.y} end end,
      AddCircleFilled=function(_,p,r) dots[#dots+1]={left=p.x-r,right=p.x+r,top=p.y-r,bottom=p.y+r} end,
      AddText_FontPtr=function(_,f,size,p,color,text) local w=font:CalcTextSizeA(size,1e9,0,text).x; texts[#texts+1]={text=text,size=size,left=p.x,right=p.x+w,top=p.y,bottom=p.y+size} end}
    local s=scene('[{:id :a :name "Plain" :fields ["field"]} {:id :b :name "Stereotyped" :stereotype :bean :ops [{:name "go" :text "go()"}]} {:id :c :name "LeveledNameIsLong" :level 3 :fields ["x"]} {:id :d :name "OnlyANameButQuiteLong"}]')
    B.new({ImVec2_Float=function(x,y) return {x=x,y=y} end,GetBackgroundDrawList_Nil=function() return list end},{body=font,title=font}).draw(s,1000,1000,{zoom=1,cam_x=0,cam_y=0})
    local function overlap(a,b) return a.left<b.right and b.left<a.right and a.top<b.bottom and b.top<a.bottom end
    local names={}; for _,c in ipairs(G.get(s,'classes')) do names[G.get(c,'name')]=G.get(c,'rect') end
    for _,t in ipairs(texts) do if names[t.text] then
      local r=names[t.text]
      assert.is_true(t.top>=G.get(r,'y') and t.bottom<=G.bottom(r),'name leaves its box: '..t.text)
      for _,l in ipairs(rules) do assert.is_false(l.left<t.right and t.left<l.right and l.y>t.top and l.y<t.bottom,'rule crosses name '..t.text) end
      for _,d in ipairs(dots) do assert.is_false(overlap(t,d),'name hits a quality dot: '..t.text) end
      for _,o in ipairs(texts) do if o~=t and o.text~=t.text then assert.is_false(overlap(t,o),t.text..' overlaps '..o.text) end end
    end end
    local sizes={}; for _,t in ipairs(texts) do if names[t.text] then sizes[t.text]=t.size end end
    assert.are.equal(18,sizes.Plain)
  end)
  it('uses native 9px C/M, 11px Level and 12px marker fonts instead of shrinking the body atlas',function()
    local drawn={}
    local list={VtxBuffer={Size=0,Data={}},AddRect=function() end,AddLine=function() end,AddCircleFilled=function() end,
      AddText_FontPtr=function(_,font,size,p,color,text) drawn[text]={font=font,size=size} end}
    local fonts={body={},title={},badge={},level={},marker={}}
    B.new({ImVec2_Float=function(x,y) return {x=x,y=y} end,GetBackgroundDrawList_Nil=function() return list end},fonts).draw(scene('[{:id :a :level 3 :stereotype :abstract}]'),1000,1000,{zoom=1})
    assert.are.same({font=fonts.badge,size=9},drawn.C)
    assert.are.same({font=fonts.badge,size=9},drawn.M)
    assert.are.same({font=fonts.level,size=11},drawn['3'])
    assert.are.same({font=fonts.marker,size=12},drawn['α'])
  end)
  it('draws the back label through the imgui draw list in the body font',function()
    local drawn={}; local list={AddText_FontPtr=function(_,f,size,point,color,text) drawn[#drawn+1]={font=f,size=size,x=point.x,y=point.y,text=text} end}
    local fonts={body={},title={}}
    B.new({ImVec2_Float=function(x,y) return {x=x,y=y} end,GetBackgroundDrawList_Nil=function() return list end},fonts).back()
    assert.are.equal('< 返回',drawn[1].text); assert.are.equal(fonts.body,drawn[1].font); assert.are.equal(18,drawn[1].size)
    assert.are.same({12,12},{drawn[1].x,drawn[1].y})
  end)
  it('paints labeled quality circles with their independent colors through cimgui',function()
    local painted,texts={},{}
    local list={AddRect=function() end,AddLine=function() end,
      AddCircleFilled=function(_,point,r,color) painted[#painted+1]={point.x,point.y,r,color} end,
      AddText_FontPtr=function(_,font,size,point,color,text) texts[text]=true end}
    B.new({ImVec2_Float=function(x,y) return {x=x,y=y} end,GetBackgroundDrawList_Nil=function() return list end}).draw(scene('[{:id :a :crap {:mu 2}}]'),1000,1000)
    assert.are.equal(0xff77bb77,painted[#painted-1] and painted[#painted-1][4]); assert.are.equal(0xff5555dd,painted[#painted] and painted[#painted][4]); assert.is_true(texts.C); assert.is_true(texts.M)
  end)
end)
