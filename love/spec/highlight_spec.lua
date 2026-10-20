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
    assert.is_true(class_cmd(boxes,'a').selected,'the parent box is selected gold')
    assert.is_true(class_cmd(boxes,'b').related); assert.is_true(class_cmd(boxes,'c').dimmed)
  end)
  it('carries input hover and selection all the way to box and edge draw commands',function()
    local Events=require('uml.application.events'); local Hit=require('uml.engine.hit')
    local s={scene=scene(),focus=E.vector(),extra=E.map(),cam_x=0,cam_y=0,zoom=1}
    local r=G.get(Hit.class_by_id(s.scene,k('a')),'rect'); local x,y=G.get(r,'x')+5,G.get(r,'y')+5
    s=Events.on_move(s,x,y)
    assert.is_true(class_cmd(B.commands(s.scene,s),'a').hovered)
    assert.is_true(edge_cmds(A.commands(s.scene,s),'a','b')[1].warm)
    s=Events.on_left_press(s,x,y); s=Events.on_release(s,x,y)
    assert.is_true(class_cmd(B.commands(s.scene,s),'a').selected)
    assert.is_true(class_cmd(B.commands(s.scene,s),'d').dimmed)
    assert.is_true(edge_cmds(A.commands(s.scene,s),'a','b')[1].hot)
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
  it('washes a selected or hovered child row and draws labeled incoming and outgoing ports with links',function()
    local s=C.compile_diagram(E.parse('{:packages [{:id :p :label "P" :classes [{:id :a :name "A" :contents [{:id :kid :name "Kid"}] :in-deps [{:id :input :name "Input"}] :out-deps [{:id :output :name "Output"}]}]}] :edges []}'))
    local cmds=B.commands(s,{hover=E.parse('{:kind :child :id :kid :parent :a}')})
    local washes,ports,links={},{},{}
    for _,c in ipairs(cmds) do if c.kind=='child-wash' then washes[#washes+1]=c elseif c.kind=='port' then ports[#ports+1]=c elseif c.kind=='port-link' then links[#links+1]=c end end
    assert.are.equal(1,#washes); assert.are.equal(T.pack(T.colors.child_wash,T.alpha.child_wash),washes[1].color)
    assert.are.equal(2,#ports); assert.are.same({'Input','Output'},{ports[1].text,ports[2].text}); assert.are.equal(2,#links)
    local box=G.get(G.get(s,'classes')[1],'rect')
    assert.are.equal(G.get(box,'y'),links[1].tip[2]); assert.are.equal(G.bottom(box),links[2].behind[2])
    local hot=B.commands(s,{hover=E.parse('{:kind :port :id :input :parent :a}')})
    local inputs={}; for _,c in ipairs(hot) do if c.kind=='port' then inputs[#inputs+1]=c end end
    assert.is_true(inputs[1].marked); assert.is_falsy(inputs[2].marked)
    local painted={}
    local list={AddLine=function() end,AddRectFilled=function() end,AddRect=function() end,AddCircleFilled=function() end,
      AddText_FontPtr=function(_,font,size,point,color,text) painted[text]=true end}
    B.new({ImVec2_Float=function(x,y) return {x=x,y=y} end,GetBackgroundDrawList_Nil=function() return list end}).draw(s,1000,1000,{zoom=.5,hover=E.parse('{:kind :port :id :input :parent :a}')})
    assert.is_true(painted.Input and painted.Output,'ports have visible labels in the actual renderer')
  end)
  it('keeps highlighted triangle arrowheads at their edge width and dims hollow fills with their edge',function()
    local s=C.compile_diagram(E.parse('{:packages [{:id :p :classes [{:id :a} {:id :b} {:id :c}]}] :edges [{:from :b :to :c :kind :inheritance :violating true}]}'))
    local strokes,fills={},{}
    local list={AddLine=function() end,AddTriangle=function(_,a,b,c,color,w) strokes[#strokes+1]=w end,AddTriangleFilled=function(_,a,b,c,color) fills[#fills+1]=color end}
    local imgui={ImVec2_Float=function(x,y) return {x=x,y=y} end}
    A.draw(imgui,list,s,0,0,.25,{selected=sel('b')}); assert.are.equal(T.widths.edge_violating_hot,strokes[1])
    fills={}; A.draw(imgui,list,s,0,0,4,{selected=sel('a')}); assert.are.equal(T.fade(T.pack(T.colors.canvas_bg)),fills[1])
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
