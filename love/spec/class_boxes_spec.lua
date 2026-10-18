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
    local styles={}; for _,c in ipairs(select_commands(s,'text')) do if c.role=='name' then styles[#styles+1]=c.italic; assert.are.equal(14,c.size) elseif c.text=='field' then assert.is_false(c.italic); assert.are.equal(12,c.size) end end
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
  it('draws the abstract alpha even when the default font has no Greek glyph',function()
    local lines={}; local list={VtxBuffer={Size=0,Data={}},AddRect=function() end,AddCircleFilled=function() end,AddText_FontPtr=function() end,
      AddLine=function(_,a,b,color) if color==0xffffffff then lines[#lines+1]={a,b} end end}
    B.new({ImVec2_Float=function(x,y) return {x=x,y=y} end,GetBackgroundDrawList_Nil=function() return list end}).draw(scene('[{:id :a :stereotype :abstract}]'),1000,1000)
    assert.is_true(#lines>=8)
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
