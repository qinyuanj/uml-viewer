local E=require('uml.edn')
local IR=require('uml.domain.ir')
local T=require('uml.domain.theme')
local C=require('uml.domain.config')
local S=require('uml.application.strings')
local Detail=require('uml.application.detail')
local V=require('uml.application.detail_view')
local function scene(text) return require('uml.engine.compose').compile_diagram(IR.normalize(E.parse(text))) end
local tiny=[[{:title "Tiny" :packages [{:id :p :label "Domain" :classes [{:id :a :name "A" :ns "demo.a" :coverage 0.9 :crap 1.2 :ops [{:name "go" :args ["x"] :returns "void" :coverage 0.75 :cc 2 :crap 1.8 :killed 3 :survived 1 :uncovered 2 :sites 6} {:name "hide" :private true :crap 4.0} {:name "bare"} {:name "low" :coverage 0.3 :killed 4 :survived 0 :uncovered 0 :sites 4}]} {:id :b :name "B"}]}] :edges [{:from :a :to :b :kind :dependency}]}]]
local function view() return V.rows(Detail.rows(Detail.model(scene(tiny),E.keyword('a')))) end
local function find(items,pred) for _,it in ipairs(items) do if pred(it) then return it end end end
local function op(items,name) return find(items,function(it) return it.source['op-name']==name end) end
local function cell(it,id) for _,c in ipairs(it.cells) do if c.id==id then return c end end end
describe('detail window view model',function()
  it('keeps the original 20px title and 18px rows at fixed right-aligned columns',function()
    local items=view(); local cols=Detail.column_layout()
    assert.are.equal(20,items[1].h); assert.are.equal('title',items[1].font)
    assert.are.equal(0,items[1].y)
    for i,it in ipairs(items) do if it.kind=='group-header' then
      assert.are.equal(6,it.y-items[i-1].y-items[i-1].h)
    end end
    local go=op(items,'go'); assert.are.equal(18,go.h); assert.are.equal('code',go.font)
    for i,col in ipairs(cols) do local c=cell(go,col.id); if c then assert.are.equal(col.right,c.right); assert.are.equal('right',c.align) end end
    local header=find(items,function(it) return it.kind=='col-header' end)
    assert.are.equal(S.member,header.text); assert.are.equal(#cols,#header.cells)
    for _,c in ipairs(header.cells) do assert.are.same(T.colors.gold,c.color); assert.is_string(S.column_tips[c.id]) end
  end)
  it('uses available detail width while retaining a readable horizontally scrollable minimum',function()
    for _,case in ipairs{{300,560},{576,576},{624,624},{900,900}} do
      local items=V.rows(Detail.rows(Detail.model(scene(tiny),E.keyword('a'))),case[1])
      local go=op(items,'go')
      assert.equal(case[2],cell(go,'uncovered').right)
      assert.is_true(go.clip_right>=130)
      local header=find(items,function(it) return it.kind=='col-header' end)
      assert.is_true(cell(header,'uncovered').right<=case[2])
    end
  end)
  it('colors cells with the original semantic rules from theme constants',function()
    local items=view(); local go=op(items,'go')
    assert.are.same(T.stroke(C.crap_grade(1.8)),cell(go,'crap').color)
    assert.are.same(T.colors.muted,cell(go,'cc').color)
    assert.are.same(T.colors.gold,cell(go,'cov').color)
    assert.are.same(T.colors.white,cell(go,'killed').color)
    assert.are.same(T.colors.violation,cell(go,'survived').color)
    assert.are.same(T.colors.violation,cell(go,'uncovered').color)
    local low=op(items,'low')
    assert.are.same(T.colors.warn,cell(low,'cov').color); assert.are.same(T.colors.good,cell(low,'survived').color)
    assert.are.same(T.colors.gold,find(items,function(it) return it.kind=='crap' end).color)
    assert.are.same(T.colors.muted,op(items,'hide').color); assert.are.same(T.colors.ink,go.color)
  end)
  it('spans the no-mutation note across the mutation group and keeps it muted',function()
    local hide=op(view(),'hide'); local groups=Detail.group_layout()
    local note=cell(hide,'mut-note'); assert.are.equal(S.no_mutation_sites,note.text)
    assert.are.same({groups[2].left,groups[2].right,'center'},{note.left,note.right,note.align}); assert.are.same(T.colors.muted,note.color)
    assert.is_nil(cell(hide,'killed'))
  end)
  it('alternates stats row backgrounds and draws Cov and kill-rate bars',function()
    local items=view(); local stats={}
    for _,it in ipairs(items) do if it.kind=='stats' then stats[#stats+1]=it end end
    for i,it in ipairs(stats) do assert.are.equal(i%2==0,it.stripe) end
    local go=op(items,'go'); local cov,kill
    for _,b in ipairs(go.bars) do if b.id=='cov' then cov=b elseif b.id=='kill' then kill=b end end
    assert.are.equal(.75,cov.fraction); assert.are.same(T.colors.gold,cov.color)
    assert.are.equal(.5,kill.fraction); assert.are.same(Detail.group_layout()[2].right,kill.right)
    assert.are.equal(0,#op(items,'bare').bars)
  end)
  it('gives every stats row a quality swatch, with an unmeasured member at the red end',function()
    local items=view()
    assert.are.same(T.stroke(1),op(items,'bare').swatch)
    local low=op(items,'low'); assert.are.same(T.stroke(C.combined_grade(C.crap_grade(nil),C.mutation_grade(1))),low.swatch)
    assert.is_nil(items[1].swatch)
  end)
  it('marks module, member and relationship rows clickable and nothing else',function()
    local items=view()
    assert.is_true(find(items,function(it) return it.source.module end).clickable)
    assert.is_true(op(items,'go').clickable)
    assert.is_true(find(items,function(it) return it.kind=='rel' end).clickable)
    assert.is_false(items[1].clickable)
  end)
end)
describe('detail window adapter',function()
  local function fake(clicked)
    local calls={}
    local vec=function(x,y) return {x=x or 0,y=y or 0} end
    local list=setmetatable({},{__index=function() return function() end end})
    local imgui=setmetatable({ImVec2_Float=vec,ImVec4_Float=function() return {} end,
      Begin=function(name) calls.begin=name; return true end,BeginChild_Str=function() return true end,
      GetCursorScreenPos=function() return vec(0,0) end,GetContentRegionAvail=function() return vec(600,600) end,
      GetWindowHeight=function() return 600 end,GetItemRectMin=function() return vec(0,0) end,GetItemRectMax=function() return vec(1,1) end,
      GetWindowDrawList=function() return list end,CalcTextSize=function(t) return vec(#t*7,13) end,
      Button=function(label) return clicked and label:find(S.close_detail,1,true)==1 end,
      InvisibleButton=function() return false end,IsItemHovered=function() return false end,IsItemClicked=function() return false end,
      IsWindowHovered=function() return false end},{__index=function(_,key) if key:match('^ImGui') then return 0 end; return function() end end})
    return imgui,calls
  end
  local Adapter=require('uml.adapters.detail_window')
  local s=require('uml.application.document').new(IR.read_document(tiny)); s.detail_id=E.keyword('d0-a')
  it('closes through the close control into the application transition',function()
    local imgui=fake(true); local closed
    Adapter.new(imgui,{close=function() closed=true end}).draw(s)
    assert.is_true(closed)
  end)
  it('paints a gold wash across the complete hovered detail row',function()
    local imgui=fake(false); local fills={}
    imgui.GetWindowDrawList=function() return setmetatable({AddRectFilled=function(_,a,b,color) fills[#fills+1]={a=a,b=b,color=color} end},{__index=function() return function() end end}) end
    imgui.IsItemHovered=function() return true end
    Adapter.new(imgui,{}).draw(s)
    local wash
    for _,fill in ipairs(fills) do if fill.color==T.pack(T.colors.gold,T.alpha.row_wash) then wash=fill end end
    assert.is_not_nil(wash)
    assert.are.equal(0,wash.a.x); assert.are.equal(600,wash.b.x)
    assert.are.equal(18,wash.b.y-wash.a.y)
  end)
  it('draws nothing while no detail target is open',function()
    local imgui,calls=fake(false); local n={}; for k,v in pairs(s) do n[k]=v end; n.detail_id=nil
    Adapter.new(imgui,{}).draw(n); assert.is_nil(calls.begin)
    Adapter.new(imgui,{close=function() error('must not close') end}).draw(s); assert.is_string(calls.begin)
  end)
end)
