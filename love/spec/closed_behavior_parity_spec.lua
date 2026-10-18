local E=require('uml.edn')
local G=require('uml.domain.geom')
local IR=require('uml.domain.ir')
local D=require('uml.application.document')
local g=G.get
local function diagram(text) return IR.normalize(E.parse(text)) end
local function first_class(text) return g(g(diagram(text),'packages')[1],'classes')[1] end
local function scene(text) return D.compile_view(IR.read_document(text)) end
local function named(xs,id) for _,x in ipairs(xs) do if G.name(g(x,'id'))==id then return x end end end

describe('compiled package quality parity',function()
  it('pools child function summaries and mutants under the plain package title',function()
    local p=g(scene('{:packages [{:id :p :label "P" :classes [{:id :a :crap {:mu 2 :max 2 :sigma 0 :n 3} :killed 9 :survived 1} {:id :b :crap {:mu 20 :max 20 :sigma 0 :n 1} :killed 1 :survived 1}]}]}'),'packages')[1]
    assert.are.equal('P',g(p,'title'))
    assert.is_true(E.equal(E.parse('{:mu 6.5 :max 20 :sigma 7.8 :n 4}'),g(p,'crap')))
    assert.are.equal(10,g(p,'killed')); assert.are.equal(2,g(p,'survived'))
  end)
  it('counts missing mutant data as one gap per child function',function()
    local p=g(scene('{:packages [{:id :p :classes [{:id :a :crap 2 :killed 9 :survived 1} {:id :b :crap {:mu 20 :n 1} :ops [{:name "a"} {:name "b"}]}]}]}'),'packages')[1]
    assert.is_true(E.equal(E.parse('{:mu 11 :max 20 :sigma 9 :n 2}'),g(p,'crap')))
    assert.are.equal(9,g(p,'killed')); assert.are.equal(1,g(p,'survived')); assert.are.equal(2,g(p,'mut-gap'))
  end)
  it('contributes a red score for a child without CRAP',function()
    local p=g(scene('{:packages [{:id :p :classes [{:id :a :crap 2 :killed 9 :survived 1} {:id :b :killed 1 :survived 1}]}]}'),'packages')[1]
    assert.is_true(E.equal(E.parse('{:mu 11 :max 20 :sigma 9 :n 2}'),g(p,'crap')))
    assert.are.equal(10,g(p,'killed')); assert.are.equal(2,g(p,'survived'))
  end)
end)

describe('closed layout behavior parity',function()
  it('stairsteps four outgoing ports while keeping their combined span narrow',function()
    local d=diagram('{:packages [{:id :p :classes [{:id :engine :name "Engine"}]}]}')
    G.put(g(g(d,'packages')[1],'classes')[1],'out-deps',E.parse('[{:id :d0 :name "Dep0"} {:id :d1 :name "Dep1"} {:id :d2 :name "Dep2"} {:id :d3 :name "Dep3"}]'))
    local ports=g(g(require('uml.engine.layout').layout(d),'classes')[1],'out-ports')
    assert.are.equal(4,#ports)
    local xs,ys,minx,maxright,sum={},{},math.huge,-math.huge,0
    for i,p in ipairs(ports) do local r=g(p,'rect'); xs[i]=g(r,'x'); ys[i]=g(r,'y'); minx=math.min(minx,xs[i]); maxright=math.max(maxright,G.right(r)); sum=sum+g(r,'w') end
    assert.is_true(ys[1]<ys[2] and ys[2]<ys[3] and ys[3]<ys[4])
    assert.is_true(xs[1]<xs[2]); assert.are.equal(xs[1],xs[3]); assert.are.equal(xs[2],xs[4])
    assert.is_true(maxright-minx<sum)
  end)
  it('stacks level zero below level two',function()
    local cs=g(scene('{:packages [{:id :p :classes [{:id :inner :level 0} {:id :outer :level 2}]}]}'),'classes')
    assert.is_true(g(g(named(cs,'d0-outer'),'rect'),'y')<g(g(named(cs,'d0-inner'),'rect'),'y'))
  end)
  it('keeps document package order when a later package holds the interface',function()
    local ps=g(scene('{:packages [{:id :impl :classes [{:id :child}]} {:id :iface :classes [{:id :parent :stereotype :interface}]}] :edges [{:from :child :to :parent :kind :implements}]}'),'packages')
    assert.is_true(g(g(ps[1],'rect'),'y')<g(g(ps[2],'rect'),'y'))
    assert.are.equal('d0-impl',G.name(g(ps[1],'id'))); assert.are.equal('d0-iface',G.name(g(ps[2],'id')))
  end)
  it('gives a five-target hub distinct channels and class attachments',function()
    local d=diagram('{:direction :lr :packages [{:id :p :classes [{:id :hub} {:id :a} {:id :b} {:id :c} {:id :d} {:id :e}]}] :edges [{:from :hub :to :a} {:from :hub :to :b} {:from :hub :to :c} {:from :hub :to :d} {:from :hub :to :e}]}')
    local s=require('uml.engine.route').route(require('uml.engine.layout').layout(d))
    local mids,starts,ends={},{},{}; local es=g(s,'edges'); assert.are.equal(5,#es)
    local function point(p) return math.floor(p[1]+0.5)..','..math.floor(p[2]+0.5) end
    for _,e in ipairs(es) do
      local ps=g(e,'points'); local mid=point(ps[2]); assert.is_nil(mids[mid]); mids[mid]=true
      local first=E.encode(g(e,'from'))..point(ps[1]); local last=E.encode(g(e,'to'))..point(ps[#ps])
      assert.is_nil(starts[first]); starts[first]=true; assert.is_nil(ends[last]); ends[last]=true
    end
  end)
end)

describe('closed geometry behavior parity',function()
  it('measures a zero-length segment as distance to its point',function()
    assert.are.equal(5,G.point_seg_dist({4,5},{1,1},{1,1}))
  end)
  it('hits a triangle interior and near edge while rejecting a distant point',function()
    local triangle={{0,0},{10,0},{5,8}}
    assert.is_true(G.near_triangle({5,2},triangle,0))
    assert.is_true(G.near_triangle({5,-1},triangle,2))
    assert.is_false(G.near_triangle({20,20},triangle,2))
  end)
  it('leaves a line that misses an obstacle intact',function()
    assert.is_true(E.equal(E.parse('[[[0 10] [100 10]]]'),G.gap_polyline(E.parse('[[0 10] [100 10]]'),{G.rect(40,40,20,20)},5)))
  end)
  it('preserves the connected bend of an unobstructed polyline',function()
    assert.is_true(E.equal(E.parse('[[[0 0] [10 0] [10 10]]]'),G.gap_polyline(E.parse('[[0 0] [10 0] [10 10]]'),{},5)))
  end)
end)

describe('closed IR behavior parity',function()
  it('preserves fractional complexity',function()
    assert.are.equal(2.5,g(first_class('{:packages [{:id :p :classes [{:id :a :cc 2.5}]}]}'),'cc'))
  end)
  for _,case in ipairs{
    {'negative complexity','{:packages [{:id :p :classes [{:id :a :cc -1}]}]}'},
    {'coverage outside zero to one','{:packages [{:id :p :classes [{:id :a :coverage 1.2}]}]}'},
    {'negative operation mutant count','{:packages [{:id :p :classes [{:id :a :ops [{:name "go" :killed -1}]}]}]}'},
    {'numeric package id','{:packages [{:id 1 :label "P" :classes [{:id :a}]}]}'},
    {'malformed CRAP','{:packages [{:id :p :crap "bad" :classes [{:id :a}]}]}'},
    {'malformed member','{:packages [{:id :p :classes [{:id :a :fields [1]}]}]}'},
    {'unnamed class','{:packages [{:id :p :classes [{}]}]}'},
    {'unnamed package','{:packages [{}]}'},
    {'missing edge endpoints','{:packages [{:id :p :classes [{:id :a}]}] :edges [{}]}'}
  } do
    it('rejects '..case[1],function() assert.has_error(function() diagram(case[2]) end) end)
  end
  it('normalizes symbol ids and the sd alias without losing sample count',function()
    local p=g(diagram('{:packages [{:id dom :crap {:mu 1 :sd 0.2 :n 4} :classes [{:id a}]}]}'),'packages')[1]
    assert.is_true(E.equal(E.keyword('dom'),g(p,'id')))
    assert.is_true(E.equal(E.keyword('a'),g(g(p,'classes')[1],'id')))
    assert.are.equal(0.2,g(g(p,'crap'),'sigma')); assert.are.equal(4,g(g(p,'crap'),'n'))
  end)
  it('builds field and operation signatures from structured members',function()
    local c=first_class('{:packages [{:id :p :classes [{:id :a :fields [{:name "n" :type "int"}] :ops [{:name "go" :args ["x"] :returns "void"}]}]}]}')
    assert.are.equal('n : int',g(g(c,'fields')[1],'text'))
    assert.are.equal('go(x) : void',g(g(c,'ops')[1],'text'))
  end)
end)
