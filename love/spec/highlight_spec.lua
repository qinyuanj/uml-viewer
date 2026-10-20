-- Selection and hover highlighting (#85), observed as canvas draw commands.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local C=require('uml.engine.compose')
local B=require('uml.adapters.class_boxes')
local A=require('uml.adapters.edges')
local T=require('uml.domain.theme')
local k=E.keyword
local function scene()
  return C.compile_diagram(E.parse('{:packages [{:id :p :label "P" :classes [{:id :a :name "A"} {:id :b :name "B"} {:id :c :name "C"} {:id :d :name "D"}]}] :edges [{:from :a :to :b :kind :dependency} {:from :b :to :c :kind :dependency :violating true}]}'))
end
local function class_cmd(cmds,id) for _,c in ipairs(cmds) do if c.kind=='class' and G.name(c.id)==id then return c end end end
local function owned(cmds,id) local out={}; for _,c in ipairs(cmds) do if c.owner_id and G.name(c.owner_id)==id then out[#out+1]=c end end; return out end
local function edge_cmds(cmds,from,to) local out={}; for _,c in ipairs(cmds) do if c.kind=='stroke' and G.name(c.from)==from and G.name(c.to)==to then out[#out+1]=c end end; return out end
local function sel(id) return G.map{kind=k('class'),id=k(id)} end
describe('selection and hover highlighting',function()
  it('selecting a class marks it, its edges and connected classes, and dims everything else',function()
    local s=scene(); local view={selected=sel('b')}
    local boxes=B.commands(s,view)
    assert.is_true(class_cmd(boxes,'b').selected); assert.is_falsy(class_cmd(boxes,'b').dimmed)
    for _,id in ipairs{'a','c'} do local c=class_cmd(boxes,id); assert.is_true(c.related,id); assert.is_falsy(c.dimmed,id) end
    assert.is_true(class_cmd(boxes,'d').dimmed)
    local texts=owned(boxes,'d'); assert.is_true(#texts>0); for _,c in ipairs(texts) do assert.is_true(c.dimmed,c.kind) end
    for _,c in ipairs(owned(boxes,'a')) do assert.is_falsy(c.dimmed,c.kind) end
    local edges=A.commands(s,view)
    for _,c in ipairs(edge_cmds(edges,'a','b')) do assert.are.equal(T.pack(T.colors.edge_hot),c.color); assert.are.equal(T.widths.edge_hot,c.width); assert.is_falsy(c.dimmed) end
    -- The violating edge keeps its violation semantics when highlighted.
    for _,c in ipairs(edge_cmds(edges,'b','c')) do assert.are.equal(T.pack(T.colors.edge_violating_hot),c.color); assert.are.equal(T.widths.edge_violating_hot,c.width) end
  end)
  it('a selection inside a class (child row or port) focuses its parent class',function()
    local s=scene()
    local boxes=B.commands(s,{selected=G.map{kind=k('port'),id=k('x'),parent=k('a')}})
    assert.is_true(class_cmd(boxes,'b').related); assert.is_true(class_cmd(boxes,'c').dimmed)
  end)
  it('dims the unrelated edges of a selection',function()
    local s=scene()
    for _,c in ipairs(edge_cmds(A.commands(s,{selected=sel('a')}),'b','c')) do assert.is_true(c.dimmed); assert.are.equal(T.pack(T.colors.edge_violating),c.color) end
  end)
  it('hover whitens the class, slightly brightens its edges and dims nothing',function()
    local s=scene(); local view={hover=sel('a')}
    local boxes=B.commands(s,view)
    assert.is_true(class_cmd(boxes,'a').hovered)
    for _,id in ipairs{'a','b','c','d'} do assert.is_falsy(class_cmd(boxes,id).dimmed,id); assert.is_falsy(class_cmd(boxes,id).related,id) end
    local edges=A.commands(s,view)
    for _,c in ipairs(edge_cmds(edges,'a','b')) do assert.is_true(c.warm); assert.are.equal(T.widths.edge_warm,c.width); assert.are.equal(T.pack(T.colors.edge_warm),c.color); assert.is_falsy(c.dimmed) end
    for _,c in ipairs(edge_cmds(edges,'b','c')) do assert.is_falsy(c.warm); assert.are.equal(T.widths.edge_violating,c.width); assert.is_falsy(c.dimmed) end
    -- Hovering a child row lifts its parent class.
    assert.is_true(class_cmd(B.commands(s,{hover=G.map{kind=k('child'),id=k('kid'),parent=k('d')}}),'d').hovered)
    assert.is_true(T.widths.edge<T.widths.edge_warm and T.widths.edge_warm<T.widths.edge_hot,'hover is a slight lift, below selection')
  end)
  it('a hovered edge is drawn hot like the original',function()
    local s=scene()
    for _,c in ipairs(edge_cmds(A.commands(s,{hover=E.parse('{:kind :edge :from :a :to :b}')}),'a','b')) do assert.are.equal(T.widths.edge_hot,c.width) end
  end)
  it('draws dimmed elements with the theme dim alpha and fixed screen-px strokes at any zoom',function()
    local s=scene(); local strokes={}
    local list={VtxBuffer={Size=0,Data={}},AddRectFilled=function() end,AddCircleFilled=function() end,AddCircle=function() end,AddText_FontPtr=function() end,AddLine=function(_,a,b,color,w) strokes[#strokes+1]={color=color,w=w} end,
      AddRect=function(_,a,b,color,r,f,w) strokes[#strokes+1]={color=color,w=w} end}
    local imgui={ImVec2_Float=function(x,y) return {x=x,y=y} end,GetBackgroundDrawList_Nil=function() return list end}
    for _,zoom in ipairs{.25,1,4} do
      strokes={}
      B.new(imgui,{}).draw(s,1000,1000,{zoom=zoom,cam_x=0,cam_y=0,selected=sel('b')})
      local alphas,widths={},{}
      for _,st in ipairs(strokes) do alphas[math.floor(st.color/16777216)]=true; widths[st.w]=true end
      assert.is_true(alphas[T.alpha.dimmed],'a dimmed class stroke')
      assert.is_true(widths[T.widths.selected] and widths[T.widths.class],'widths stay screen px at zoom '..zoom)
    end
  end)
end)
