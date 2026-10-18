local E=require('uml.edn')
local G=require('uml.domain.geom')
local g=G.get
local function read(p) local f=assert(io.open(p)); local t=f:read('*a'); f:close(); return E.parse(t) end
local function close(a,b,path)
  path=path or ''
  if type(a)=='number' then assert.is_true(type(b)=='number' and b==b and math.abs(a-b)<=1e-6,path..' expected '..tostring(a)..' actual '..tostring(b))
  elseif E.kind(a)=='map' then assert.are.equal('map',E.kind(b),path); assert.are.equal(#a,#b,path); for _,kv in ipairs(a) do close(kv[2],E.get(b,kv[1]),path..E.encode(kv[1])) end
  elseif E.kind(a)=='vector' then assert.are.equal('vector',E.kind(b),path); assert.are.equal(#a,#b,path); for i,v in ipairs(a) do close(v,b[i],path..'['..i..']') end
  else assert.is_true(E.equal(a,b),path..' expected '..E.encode(a)..' actual '..E.encode(b)) end
end
describe('viewer edge scene parity',function()
  it('matches every edge and constrained control point in all four JVM geometry goldens',function()
    local C=require('uml.engine.compose'); local H=require('uml.domain.hierarchy'); local Curve=require('uml.engine.curve')
    for _,lang in ipairs{'lua','typescript','rust','python'} do
      local doc=read('fixtures/jvm-goldens/'..lang..'.ir.edn')
      for _,view in ipairs(g(read('fixtures/jvm-goldens/'..lang..'.geometry.edn'),'views')) do
        local scene=C.compile_diagram(H.view_at(doc,g(view,'focus')))
        local actual=E.vector(); local idx={}; for _,c in ipairs(g(scene,'classes')) do idx[E.encode(g(c,'id'))]=g(c,'rect') end
        for _,e in ipairs(g(scene,'edges')) do
          local p=G.map{}; for _,k in ipairs{'from','to','kind','points','tip','behind','draw-bounds'} do G.put(p,k,g(e,k)) end
          G.put(p,'curve',Curve.constrain_ends(Curve.basis_path(g(e,'points')),idx[E.encode(g(e,'from'))],idx[E.encode(g(e,'to'))])); actual[#actual+1]=p
        end
        close(g(view,'edges'),actual,lang..E.encode(g(view,'focus')))
      end
    end
  end)
  it('selects ports before classes and the last edge with leaf dependencies',function()
    local Hit=require('uml.engine.hit')
    local s=E.parse('{:classes [{:id :a :rect {:x 0 :y 0 :w 40 :h 20} :in-ports [{:id :p :rect {:x 0 :y 0 :w 20 :h 18}}]}] :packages [] :edges [{:from :a :to :b :points [[0 50] [100 50]]} {:from :x :to :y :points [[0 50] [100 50]] :strokes []}]}')
    close(E.parse('{:kind :port :id :p :parent :a :dir :in}'),Hit.at(s,10,10))
    close(E.parse('{:kind :edge :from :x :to :y :deps [{:from :x :to :y :violating false}]}'),Hit.at(s,50,50))
  end)
  it('keeps collapsed leaf indicators above overlapping classes and excludes dummy boxes',function()
    local Hit=require('uml.engine.hit')
    local s=E.parse('{:classes [{:id :child :rect {:x 10 :y 10 :w 40 :h 20}} {:id :container :dummy? true :rect {:x 0 :y 0 :w 80 :h 80}} {:id :other :rect {:x 0 :y 120 :w 40 :h 20}}] :packages [] :edges [{:from :container :to :other :deps [{:from :child :to :leaf :violating true}]}]}')
    G.put(s,'dep-indicators',Hit.dep_indicators(s))
    local h=Hit.at(s,30,38); assert.are.equal('dep',G.name(g(h,'kind'))); assert.is_true(E.equal(E.keyword('child'),g(h,'id'))); assert.is_true(g(h,'violating'))
    assert.is_nil(Hit.at(s,75,75)); assert.are.equal(1,#Hit.connected_edges(s,E.keyword('container'))); assert.are.equal(0,#Hit.connected_edges(s,E.keyword('child')))
    assert.is_not_nil(Hit.class_by_id(s,E.keyword('child'))); assert.is_nil(Hit.class_by_id(s,E.keyword('missing')))
  end)
  it('selects child rows, outgoing ports, class boundaries and package frames',function()
    local Hit=require('uml.engine.hit')
    local s=E.parse('{:classes [{:id :a :drill? true :rect {:x 10 :y 10 :w 100 :h 100} :lines [{:kind :text :text "A"} {:kind :rule} {:kind :child :id :leaf :drill? true}] :out-ports [{:id :p :rect {:x 20 :y 130 :w 40 :h 18}}]}] :packages [{:id :pkg :rect {:x 0 :y 0 :w 200 :h 200}}] :edges []}')
    close(E.parse('{:kind :child :id :leaf :parent :a :drill? true}'),Hit.at(s,20,56))
    close(E.parse('{:kind :class :id :a :drill? true}'),Hit.at(s,110,110))
    close(E.parse('{:kind :port :id :p :parent :a :dir :out}'),Hit.at(s,30,140))
    close(E.parse('{:kind :package :id :pkg}'),Hit.at(s,1,1)); assert.is_not_nil(Hit.package_by_id(s,E.keyword('pkg')))
  end)
  it('clips merged obstacles with JVM padding and never selects the hidden stroke gap',function()
    local C=require('uml.engine.compose'); local Hit=require('uml.engine.hit')
    local s=C.prepare_scene(E.parse('{:classes [{:id :wall :rect {:x 40 :y -10 :w 20 :h 20}}] :packages [] :edges [{:from :a :to :b :points [[0 0] [100 0]]}]}'))
    assert.is_nil(Hit.edge_at(s,50,0,1)); assert.is_not_nil(Hit.edge_at(s,20,0,1))
    close(E.parse('[[[0 0] [35 0]] [[75 0] [100 0]]]'),G.gap_polyline(E.parse('[[0 0] [100 0]]'),{G.rect(40,-10,20,20),G.rect(55,-10,15,20)},5))
    assert.are.equal(0,#G.gap_segment({0,0},{.5,0},{},0))
  end)
  it('preserves short curves, chamfer insets, pinned final tangent and control hull',function()
    local C=require('uml.engine.curve')
    close(E.parse('{:start [0 0] :ops []}'),C.basis_path(E.vector()))
    close(E.parse('{:start [2 3] :ops []}'),C.basis_path(E.parse('[[2 3]]')))
    close(E.parse('[[0 0] [0 56] [24 80] [80 80]]'),C.round_corners(E.parse('[[0 0] [0 80] [80 80]]')))
    local p=C.basis_path(E.parse('[[0 0] [40 0] [40 80] [120 80]]')); local t=C.end_tangent(p); close(E.parse('[120 80]'),t[2]); assert.is_true(math.abs(t[2][2]-t[1][2])>1)
    local samples=C.flatten_path(p); close(E.parse('[0 0]'),samples[1]); close(E.parse('[120 80]'),samples[#samples]); assert.is_true(#samples>4)
    close(G.rect(-80,0,80,80),C.path_bounds(E.parse('{:start [0 0] :ops [{:op :cubic :c1 [-80 10] :c2 [-80 70] :p [0 80]}]}')))
  end)
  it('drops unlaid endpoints and preserves supplied edge metadata and head kinds',function()
    local R=require('uml.engine.route')
    local s=E.parse('{:diagram {:edges [{:from :a :to :missing} {:from :a :to :b :kind :implements :dashed? true}]} :classes [{:id :a :rect {:x 0 :y 0 :w 40 :h 20}} {:id :b :rect {:x 0 :y 80 :w 40 :h 20}} {:id :missing}]}')
    R.route(s); assert.are.equal(1,#g(s,'edges')); local e=g(s,'edges')[1]; assert.are.equal('triangle',G.name(g(e,'head'))); assert.is_true(g(e,'dashed?'))
  end)
  it('uses the last equally smooth clear detour and first blocked fallback',function()
    local R=require('uml.engine.route')
    local s=E.parse('{:diagram {:edges [{:from :a :to :b}]} :classes [{:id :a :rect {:x 0 :y 0 :w 40 :h 20}} {:id :b :rect {:x 80 :y 0 :w 40 :h 20}} {:id :wall :rect {:x 50 :y 0 :w 20 :h 20}}]}')
    R.route(s); close(E.parse('[[20 20] [20 40] [100 40] [100 20]]'),g(g(s,'edges')[1],'points'))
    G.put(g(s,'classes')[3],'rect',G.rect(-100,-100,400,400)); R.route(s)
    close(E.parse('[[40 10] [80 10]]'),g(g(s,'edges')[1],'points'))
  end)
  it('stacks flat diagrams with qualified edge endpoints and prepared strokes',function()
    local C=require('uml.engine.compose'); local IR=require('uml.domain.ir')
    local s=C.compile_document(IR.read_document('{:title "D" :diagrams [{:packages [{:id :p :label "P" :classes [{:id :a} {:id :b}]}] :edges [{:from :a :to :b}]} {:packages [{:id :p :label "P" :classes [{:id :a} {:id :b}]}] :edges [{:from :a :to :b}]}]}'))
    assert.are.equal(2,#g(s,'sections')); assert.are.equal(2,#g(s,'edges'))
    for i,e in ipairs(g(s,'edges')) do assert.is_true(E.equal(E.keyword('d'..(i-1)..'-a'),g(e,'from'))); assert.is_true(E.equal(E.keyword('d'..(i-1)..'-b'),g(e,'to'))); assert.is_true(#g(e,'strokes')>0) end
    local first,second=g(s,'edges')[1],g(s,'edges')[2]; assert.is_true(g(second,'tip')[2]>g(first,'tip')[2]); assert.is_true(g(g(s,'size'),'h')>g(second,'tip')[2])
  end)
  it('hides edge strokes in favor of incoming and outgoing indicators',function()
    local C=require('uml.engine.compose'); local A=require('uml.adapters.edges')
    local s=C.compile_diagram(E.parse('{:hide-edges true :packages [{:id :p :label "P" :classes [{:id :a :name "A"} {:id :b :name "B"}]}] :edges [{:from :a :to :b :kind :dependency :violating true}]}'))
    assert.are.equal(2,#g(s,'dep-indicators')); local commands=A.commands(s); assert.are.equal(2,#commands); for _,cmd in ipairs(commands) do assert.are.equal('indicator',cmd.kind); assert.is_true(cmd.violating) end
  end)
  it('rotates shallow departure handles and arrival tangents onto each face cone',function()
    local C=require('uml.engine.curve'); local r=G.rect(0,0,100,40)
    for _,case in ipairs{{{100,20},{100,80},1},{{0,20},{0,80},1},{{50,0},{110,0},2},{{50,40},{110,40},2}} do
      local p,q,axis=case[1],case[2],case[3]
      local raw=G.map{start=E.vector(p),ops=E.vector{G.map{op=E.keyword('cubic'),c1=E.vector(q),c2=E.vector(q),p=E.vector(p)},G.map{op=E.keyword('line'),p=E.vector{160,20}}}}
      local constrained=C.constrain_ends(raw,r,nil); local handle=g(g(constrained,'ops')[1],'c1'); local dx,dy=handle[1]-p[1],handle[2]-p[2]; assert.is_true(math.abs(math.abs(dx)-math.abs(dy))<1e-6); assert.is_true(math.abs(dx*dx+dy*dy-3600)<1e-6)
      assert.is_true(E.equal(E.vector(q),g(g(raw,'ops')[1],'c1')))
    end
    local path=C.basis_path(E.parse('[[0 0] [10 0] [10 400] [40 400]]')); local t=C.end_tangent(C.constrain_ends(path,nil,G.rect(40,360,80,80)))
    assert.is_true(math.abs(t[2][1]-t[1][1])+1e-6>=math.abs(t[2][2]-t[1][2])); close(E.parse('[40 400]'),t[2])
  end)
  it('renders clipped strokes and oriented heads through the cimgui draw-list contract',function()
    local A=require('uml.adapters.edges'); local C=require('uml.engine.compose')
    local s=C.prepare_scene(E.parse('{:diagram {} :packages [] :classes [{:id :a :rect {:x 0 :y 0 :w 20 :h 20}} {:id :b :rect {:x 180 :y 0 :w 20 :h 20}} {:id :wall :rect {:x 80 :y 0 :w 40 :h 20}}] :edges [{:from :a :to :b :head :triangle :points [[20 10] [180 10]]}]}'))
    local commands=A.commands(s); assert.are.equal(3,#commands); assert.are.equal('stroke',commands[1].kind); close(E.parse('[[20 10] [75 10]]'),commands[1].points); close(E.parse('[[125 10] [180 10]]'),commands[2].points); assert.are.equal('triangle',commands[3].head)
    local lines,triangles=0,0
    local draw={AddLine=function(_,a,b,color,w) assert.is_number(a.x); assert.is_number(w); lines=lines+1 end,AddTriangleFilled=function() triangles=triangles+1 end,AddTriangle=function() triangles=triangles+1 end}
    local imgui={ImVec2_Float=function(x,y) return {x=x,y=y} end}
    A.draw(imgui,draw,s,0,0,1); assert.are.equal(2,lines); assert.are.equal(2,triangles)
  end)
end)
