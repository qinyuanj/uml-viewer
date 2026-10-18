local E=require('uml.edn')
local G=require('uml.domain.geom')
local D=require('uml.application.document')
local IR=require('uml.domain.ir')
local B=require('uml.adapters.class_boxes')
local g=G.get
local function parse(s) return E.parse(s) end
local function scene(doc,files) return D.compile_view(IR.read_document(doc),files) end
local function classes(s) return g(s,'classes') end
local function red(s) local r={}; for _,c in ipairs(B.commands(s)) do if c.kind=='class' then r[#r+1]=c.red end end; return r end
local function snapshot(path,text,mtime) return {path=path,text=text,mtime=mtime or '1',size=#text} end
local fixture='{:packages [{:label "P" :classes [{:id :calc :ns "calc.util" :ops [{:name "M.clamp" :text "authored signature"}]}]}]}'
describe('viewer metrics overlay',function()
  it('paints standard Lua snapshots including private forms and percent coverage',function()
    local files={snapshot('.metrics/crap.edn','{:entries [{:namespace "calc.util" :name "M.clamp" :complexity 2 :coverage 83.3333 :crap 3}]}'),snapshot('.metrics/mutate/calc.edn','{:namespace "calc.util" :forms [{:id "defn/M.clamp" :killed 4 :survived 1 :uncovered 2} {:id "defn-/between" :killed 1 :survived 0 :sites 0}]}')}
    local s=scene(fixture,files); local c=classes(s)[1]
    assert.are.equal(3,g(g(c,'crap'),'mu')); assert.are.equal(.833333,g(c,'coverage'))
    assert.are.equal(5,g(c,'killed')); assert.are.equal(1,g(c,'survived')); assert.are.equal(7,g(c,'sites'))
    assert.are.equal('authored signature',g(g(c,'ops')[1],'text'))
    assert.is_true(g(g(c,'ops')[2],'private')); assert.are.equal(0,g(g(c,'ops')[2],'sites'))
  end)
  it('keeps missing, uncovered-only and measured-zero adapter classes red',function()
    local doc='{:packages [{:label "P" :classes [{:id :ffi :ns "ffi"} {:id :zero :ns "zero"} {:id :uncov :ns "uncov"} {:id :ok :ns "ok"}]}]}'
    local files={snapshot('.metrics/crap.edn','{:entries [{:namespace "zero" :name "f" :complexity 0 :coverage 0 :crap 0} {:namespace "uncov" :name "f" :complexity 1 :crap 1} {:namespace "ok" :name "f" :complexity 1 :crap 1}]}'),snapshot('.metrics/mutate/zero.edn','{:namespace "zero" :forms [{:id "defn/f" :killed 0 :survived 0}]}'),snapshot('.metrics/mutate/uncov.edn','{:namespace "uncov" :forms [{:id "defn/f" :uncovered 7}]}'),snapshot('.metrics/mutate/ok.edn','{:namespace "ok" :forms [{:id "defn/f" :killed 2 :survived 0}]}')}
    assert.are.same({true,true,true,false},red(scene(doc,files)))
  end)
  it('reloads metric-only changes from the base IR and retains focus',function()
    local files={snapshot('.metrics/crap.edn','{:entries [{:namespace "calc.util" :name "f" :complexity 1 :crap 1}]}')}
    local state=D.new(IR.read_document(fixture),'ir',nil,nil,files,'metrics-1')
    assert.are.equal(1,g(g(classes(state.scene)[1],'crap'),'mu'))
    files[1]=snapshot('.metrics/crap.edn','{:entries [{:namespace "calc.util" :name "f" :complexity 9 :crap 9}]}','2')
    local next_state,changed=D.reload(state,fixture,'ir',files,'metrics-2')
    assert.is_true(changed); assert.are.equal(2,next_state.revision); assert.are.equal(state.focus,next_state.focus)
    assert.are.equal(9,g(g(classes(next_state.scene)[1],'crap'),'mu'))
    local cleared,removed=D.reload(next_state,fixture,'ir',{},'metrics-3')
    assert.is_true(removed); assert.is_nil(g(classes(cleared.scene)[1],'crap'))
    local retained,accepted,err=D.reload(cleared,fixture,'ir',{snapshot('.metrics/crap.edn','{')},'metrics-4')
    assert.are.equal(cleared,retained); assert.is_false(accepted); assert.is_string(err)
  end)
  it('assigns Rust children to nearest id while exact namespace wins',function()
    local doc=parse('{:hierarchical true :prefix :crate :classes [{:id :root :ns "root" :lang :rust} {:id :model :ns "my_model"} {:id :model.deep :ns "deep"} {:id :other :ns "model.deep.Exact"}]}')
    local files={snapshot('.metrics/crap.edn','{:entries [{:namespace "my_model" :name "a" :complexity 1 :crap 1} {:namespace "model" :name "b" :complexity 2 :crap 2} {:namespace "model::deep::Child" :name "c" :complexity 3 :crap 3} {:namespace "model.deep.Exact" :name "d" :complexity 4 :crap 4} {:namespace "crate" :name "e" :complexity 5 :crap 5}]}')}
    local cs=g(require('uml.application.overlay').apply(doc,files),'classes')
    assert.are.same({5,3,3,4},{g(cs[1],'cc'),g(cs[2],'cc'),g(cs[3],'cc'),g(cs[4],'cc')})
  end)
  it('loads real standard-tool Lua fixtures through the scanned namespace',function()
    local function file(p) local f=assert(io.open(p,'rb')); local t=f:read('*a'); f:close(); return snapshot(p,t) end
    local files={file('lua-fixture/.metrics/crap.edn'),file('lua-fixture/.metrics/mutate/calc.util.edn')}
    local c=classes(scene(fixture:gsub(':id :calc ',':id :util '),files))[1]
    assert.are.equal(4,g(g(c,'crap'),'mu')); assert.are.equal(3,g(c,'killed')); assert.are.equal(3,g(c,'survived')); assert.are.equal(2,g(c,'uncovered')); assert.are.equal(8,g(c,'sites')); assert.is_true(math.abs(g(c,'coverage')-.6666666666666667)<1e-12)
    local ops={}; for _,op in ipairs(g(c,'ops')) do ops[g(op,'name')]=op end
    assert.is_true(g(ops.between,'private')); assert.are.equal(0,g(ops['M.untested'],'coverage')); assert.are.equal(0,g(ops['M.untested'],'killed'))
  end)
  it('IO adapter discovers parent metrics and fingerprints add, change and delete',function()
    local data={['/p/.metrics/crap.edn']='{:entries []}', ['/p/.metrics/mutate/nested/a.edn']='{:namespace "a"}'}
    local times={['/p/.metrics/crap.edn']='1',['/p/.metrics/mutate/nested/a.edn']='2'}
    local directories={['/p']={{name='.metrics',directory=true}},['/p/diagrams']={},['/p/.metrics']={{name='crap.edn'},{name='mutate',directory=true}},['/p/.metrics/mutate']={{name='nested',directory=true}},['/p/.metrics/mutate/nested']={{name='a.edn'}}}
    local host={canonical=function(p) return p end,entries=function(p) return assert(directories[p],p) end,read=function(p) return assert(data[p]) end,mtime=function(p) return assert(times[p]) end,cwd=function() return '/fallback' end}
    local adapter=require('uml.adapters.metrics').new(host)
    assert.are.equal('/p',adapter.root('/p/diagrams/view.edn'))
    local files,stamp=adapter.snapshot('/p'); assert.are.equal(2,#files)
    times['/p/.metrics/crap.edn']='3'; local _,changed=adapter.snapshot('/p'); assert.are_not.equal(stamp,changed)
    directories['/p/.metrics/mutate/nested']={}; local removed,deleted=adapter.snapshot('/p'); assert.are.equal(1,#removed); assert.are_not.equal(changed,deleted)
  end)
  it('uses the newest duplicate namespace snapshot with numeric file times and preserves underscores',function()
    local files={snapshot('.metrics/mutate/new.edn','{:source "src/ignored.lua" :namespace "calc.util" :forms [{:id "defn/f" :killed 7}]}','10'),snapshot('.metrics/mutate/old.edn','{:namespace "calc.util" :forms [{:id "defn/f" :killed 1}]}','9')}
    assert.are.equal(7,g(classes(scene(fixture,files))[1],'killed'))
    files={snapshot('.metrics/mutate/a.edn','{:namespace "my_module" :source "src/wrong.lua" :forms [{:id "defn/f" :killed 2}]}')}
    local s=scene('{:packages [{:label "P" :classes [{:id :a :ns "my_module"}]}]}',files)
    assert.are.equal(2,g(classes(s)[1],'killed'))
  end)
  it('preserves authored fallback and ignores foreign snapshots across stacked diagrams',function()
    local text='{:diagrams [{:packages [{:label "P" :classes [{:id :a :ns "a" :crap 2 :killed 1 :survived 0} {:id :b :ns "b" :shape :oval}]}]} {:packages [{:label "Q" :classes [{:id :c :ns "c"}]}]}]}'
    local files={snapshot('.metrics/crap.edn','{:entries [{:namespace "b" :name "f" :complexity 5 :crap 5} {:namespace "c" :name "f" :complexity 7 :crap 7}]}')}
    local cs=classes(scene(text,files))
    assert.are.equal(2,g(g(cs[1],'crap'),'mu')); assert.is_nil(g(cs[2],'crap')); assert.are.equal(7,g(g(cs[3],'crap'),'mu'))
    local raw=IR.read_document(text); assert.are.equal(raw,require('uml.application.overlay').apply(raw,{}))
  end)
  it('reports risk and mutation threshold colors without treating sites as denominator',function()
    local text='{:packages [{:label "P" :classes [{:id :a :crap 8 :killed 9 :survived 1 :uncovered 100 :sites 110} {:id :b :crap 12 :killed 10 :survived 0} {:id :c :crap 13 :killed 10 :survived 0} {:id :d :killed 10 :survived 0}]}]}'
    local bands={}; for _,cmd in ipairs(B.commands(scene(text))) do if cmd.kind=='class' then bands[#bands+1]=cmd.band end end
    assert.are.same({'yellow','yellow','red','red'},bands)
  end)
end)
