local E=require('uml.edn')
local G=require('uml.domain.geom')
local IR=require('uml.domain.ir')
local D=require('uml.application.document')
local B=require('uml.adapters.class_boxes')
local T=require('uml.domain.theme')
local function scene(packages) return D.compile_view(IR.read_document('{:packages '..packages..'}')) end
local function of(s,kind) local xs={}; for _,c in ipairs(B.commands(s)) do if c.kind==kind then xs[#xs+1]=c end end; return xs end
describe('no-data versus poor quality',function()
  it('marks an unmeasured class missing and keeps it red; a measured red class is not missing',function()
    local s=scene('[{:label "P" :classes [{:id :none :name "None"} {:id :bad :name "Bad" :crap {:mu 40} :killed 1 :survived 9}]}]')
    local by={}; for _,c in ipairs(of(s,'class')) do by[(G.name(c.id):gsub('^d0%-',''))]=c end
    assert.is_true(by.none.missing); assert.are.equal('red',by.none.band)
    assert.is_false(by.bad.missing); assert.are.equal('red',by.bad.band)
    local markers=of(s,'no-data'); assert.are.equal(1,#markers); assert.are.equal('无数据',markers[1].text)
  end)
  it('aggregates missing per package without inferring it from the red band',function()
    local s=scene('[{:id :empty :label "Empty" :classes [{:id :a} {:id :b}]} {:id :mixed :label "Mixed" :classes [{:id :c :crap {:mu 2} :killed 9 :survived 0} {:id :d}]} {:id :poor :label "Poor" :classes [{:id :e :crap {:mu 40} :killed 1 :survived 9}]}]')
    local by={}; for _,p in ipairs(of(s,'package')) do by[p.text]=p end
    assert.is_true(by.Empty.missing); assert.are.equal('red',by.Empty.band)
    assert.is_true(by.Mixed.missing); assert.are.equal(1,by.Mixed.missing_count)
    assert.is_false(by.Poor.missing); assert.are.equal('red',by.Poor.band)
  end)
  it('treats either absent metric as missing: red, dashed and marked, never averaged up',function()
    local s=scene('[{:label "P" :classes [{:id :crap-only :name "C" :crap {:mu 2}} {:id :mut-only :name "M" :killed 9 :survived 0}]}]')
    for _,c in ipairs(of(s,'class')) do
      assert.is_true(c.missing,G.name(c.id)); assert.are.equal(1,c.grade)
      assert.are.same(T.fill(1),c.fill); assert.are.same(T.stroke(1),c.stroke)
    end
    assert.are.equal(2,#of(s,'no-data'))
  end)
end)
describe('quality gradient',function()
  it('restores the historical Detail yellow from measured grades without changing its red risk band',function()
    local s=scene('[{:label "P" :classes [{:id :d :name "Detail" :crap {:mu 6.6 :sigma 9.2} :killed 135 :survived 19}]}]')
    local c=of(s,'class')[1]
    assert.are.equal('red',c.band); assert.is_false(c.missing)
    assert.near(3.905275974026,c.grade,1e-10)
    assert.are.same({215,177,86},c.stroke)
  end)
  it('fills and strokes a measured class from the averaged CRAP and mutation grade',function()
    local C=require('uml.domain.config')
    -- Same shape as Detail: CRAP band red (mu+sigma 19) but mutation 0.89 yellow.
    local s=scene('[{:label "P" :classes [{:id :d :name "Detail" :crap {:mu 14 :sigma 5} :killed 154 :survived 19}]}]')
    local c=of(s,'class')[1]
    local grade=C.combined_grade(C.crap_grade(19),C.mutation_grade(154/173))
    assert.are.equal('red',c.band); assert.is_false(c.missing)
    assert.are.equal(grade,c.grade); assert.is_true(c.grade>1)
    assert.are.same(T.fill(grade),c.fill); assert.are.same(T.stroke(grade),c.stroke)
  end)
  it('paints the gradient fill, rounded border, 2px title band and per-metric C/M ramps',function()
    local C=require('uml.domain.config')
    local s=scene('[{:label "P" :classes [{:id :a :name "A" :crap {:mu 2} :killed 9 :survived 1 :fields ["f"]}]}]')
    local fills,rects,lines,dots={},{},{},{}
    local list={VtxBuffer={Size=0,Data={}},AddText_FontPtr=function() end,
      AddRectFilled=function(_,a,b,color,r) fills[#fills+1]={color=color,r=r} end,
      AddRect=function(_,a,b,color,r,f,w) rects[#rects+1]={color=color,r=r,width=w} end,
      AddLine=function(_,a,b,color,w) lines[#lines+1]={color=color,width=w,y=a.y} end,
      AddCircleFilled=function(_,p,r,color) dots[#dots+1]={fill=color} end,AddCircle=function(_,p,r,color,seg,w) dots[#dots].stroke=color; dots[#dots].width=w end}
    B.new({ImVec2_Float=function(x,y) return {x=x,y=y} end,GetBackgroundDrawList_Nil=function() return list end}).draw(s,1000,1000,{zoom=2,cam_x=0,cam_y=0})
    local c=of(s,'class')[1]
    assert.are.equal(T.pack(T.fill(c.grade)),fills[#fills].color); assert.are.equal(T.radius.class*2,fills[#fills].r)
    assert.are.equal(T.pack(T.stroke(c.grade)),rects[#rects].color); assert.are.equal(T.widths.class,rects[#rects].width)
    assert.are.equal(T.pack(T.fill(c.grade),T.alpha.package_fill),fills[1].color)
    local band=false; for _,l in ipairs(lines) do if l.color==T.pack(T.stroke(c.grade)) and l.width==T.widths.band then band=true end end
    assert.is_true(band,'2px title band in the class stroke color')
    local cg,mg=C.crap_grade(2),C.mutation_grade(.9)
    assert.are.same({T.pack(T.fill(cg)),T.pack(T.stroke(cg)),T.widths.dot},{dots[3].fill,dots[3].stroke,dots[3].width})
    assert.are.equal(T.pack(T.fill(mg)),dots[4].fill)
  end)
end)
describe('theme line widths',function()
  local function draw(zoom,selected)
    local s=scene('[{:label "P" :classes [{:id :a :name "A" :crap {:mu 2} :killed 9 :survived 0}]} {:label "Q" :classes [{:id :b :name "B"}]}]')
    local rects,lines={},{}
    local list={VtxBuffer={Size=0,Data={}},AddRectFilled=function() end,AddCircleFilled=function() end,AddText_FontPtr=function() end,
      AddRect=function(_,a,b,color,r,f,w) rects[#rects+1]={color=color,width=w} end,
      AddLine=function(_,a,b,color,w) lines[#lines+1]={color=color,width=w} end}
    B.new({ImVec2_Float=function(x,y) return {x=x,y=y} end,GetBackgroundDrawList_Nil=function() return list end}).draw(s,1000,1000,{zoom=zoom,cam_x=0,cam_y=0,selected=selected})
    return rects,lines
  end
  it('draws package, class, selected and dashed no-data borders at fixed screen px for every zoom',function()
    for _,zoom in ipairs{.25,1,3} do
      local rects,lines=draw(zoom,G.map{kind=E.keyword('class'),id=E.keyword('d0-a')})
      assert.are.same({T.widths.package,T.widths.selected},{rects[1].width,rects[2].width})
      assert.are.equal(T.pack(T.colors.gold),rects[2].color)
      assert.are.equal(2,#rects,'missing package and class borders are dashed, not solid')
      local dashed,frame=0,0
      for _,l in ipairs(lines) do
        if l.color==T.fade(T.pack(T.stroke(1))) and l.width==T.widths.class then dashed=dashed+1 end
        if l.width==T.widths.package then frame=frame+1 end
      end
      assert.is_true(dashed>4,'no-data class border is dashed at zoom '..zoom)
      assert.is_true(frame>4,'namespace with a missing member is dashed at zoom '..zoom)
    end
  end)
  it('draws edges at fixed theme widths at every zoom',function()
    local A=require('uml.adapters.edges'); local C=require('uml.engine.compose')
    local s=C.compile_diagram(E.parse('{:packages [{:id :p :label "P" :classes [{:id :a :name "A"} {:id :b :name "B"}]}] :edges [{:from :a :to :b :kind :dependency} {:from :b :to :a :kind :dependency :violating true}]}'))
    for _,zoom in ipairs{.25,1,3} do
      local widths={}
      A.draw({ImVec2_Float=function(x,y) return {x=x,y=y} end},{AddLine=function(_,a,b,c,w) widths[w]=true end,AddTriangle=function() end,AddTriangleFilled=function() end},s,0,0,zoom)
      assert.is_true(widths[T.widths.edge]); assert.is_true(widths[T.widths.edge_violating])
      local hot={}; A.draw({ImVec2_Float=function(x,y) return {x=x,y=y} end},{AddLine=function(_,a,b,c,w) hot[w]=true end,AddTriangle=function() end,AddTriangleFilled=function() end},s,0,0,zoom,function() return true end)
      assert.is_true(hot[T.widths.edge_hot]); assert.is_true(hot[T.widths.edge_violating_hot])
    end
  end)
  it('keeps the ticket baseline widths behind a single DPI entry point',function()
    assert.are.same({2.5,2,3.5,1.6,2.8,2.4},{T.widths.class,T.widths.package,T.widths.selected,T.widths.edge,T.widths.edge_hot,T.widths.edge_violating})
    assert.are.equal(5,T.dpi(2).class); assert.are.equal(2.5,T.widths.class)
  end)
end)
