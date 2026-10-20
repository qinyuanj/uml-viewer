local E=require('uml.edn')
local function get(m,k) local v=E.get(m,E.keyword(k)); return v end
local function read(path) local f=assert(io.open(path)); local s=f:read('*a'); f:close(); return E.parse(s) end
local function close_rect(expected,actual)
  assert.is_not_nil(actual)
  for _,k in ipairs{'x','y','w','h'} do assert.is_true(math.abs(get(expected,k)-get(actual,k))<=1e-6,k..' expected '..E.encode(expected)..' actual '..E.encode(actual)) end
end
describe('viewer application class boxes',function()
  it('matches independent JVM class rectangles at every captured namespace focus',function()
    local D=require('uml.application.document')
    for _,lang in ipairs{'lua','typescript','rust','python'} do
      local doc=read('fixtures/jvm-goldens/'..lang..'.ir.edn')
      local golden=read('fixtures/jvm-goldens/'..lang..'.geometry.edn')
      for _,view in ipairs(get(golden,'views')) do
        local scene=D.compile_view(doc,nil,get(view,'focus'))
        local packages=get(scene,'packages'); assert.are.equal(#get(view,'packages'),#packages)
        for i,p in ipairs(get(view,'packages')) do close_rect(get(p,'rect'),get(packages[i],'rect')) end
        for _,k in ipairs{'w','h','min-x','min-y'} do assert.is_true(math.abs(get(get(view,'size'),k)-get(get(scene,'size'),k))<=1e-6,lang..' size '..k) end
        local classes=get(scene,'classes'); assert.are.equal(#get(view,'classes'),#classes)
        for i,c in ipairs(get(view,'classes')) do
          assert.is_true(E.equal(get(c,'id'),get(classes[i],'id')),lang..' '..E.encode(get(view,'focus'))..' expected '..E.encode(get(c,'id'))..' actual '..E.encode(get(classes[i],'id')))
          close_rect(get(c,'rect'),get(classes[i],'rect'))
        end
      end
    end
  end)
  it('reloads changed IR and retains the last valid scene on invalid edits',function()
    local D=require('uml.application.document'); local IR=require('uml.domain.ir')
    local first='{:packages [{:label "P" :classes [{:id :a :ops ["public" {:text "secret" :private true}]}]}]}'
    local state=D.new(IR.read_document(first),'one')
    local unchanged,changed=D.reload(state,first,'one'); assert.is_false(changed); assert.are.equal(state,unchanged)
    local next_state,reloaded=D.reload(state,first:gsub('public','long public method'),'two')
    assert.is_true(reloaded); assert.are.equal(2,next_state.revision)
    local retained,accepted,err=D.reload(next_state,'{','three'); assert.is_false(accepted); assert.are.equal(next_state,retained); assert.is_string(err)
  end)
  it('validates retained namespace focus and card targets on automatic IR and metrics reload',function()
    local D=require('uml.application.document'); local IR=require('uml.domain.ir')
    local first='{:hierarchical true :classes [{:id :alpha.deep.parser} {:id :alpha.other} {:id :beta.parser}] :edges []}'
    local state=D.new(IR.read_document(first),'one',E.parse('[:alpha :deep]'))
    state.cam_x=71; state.cam_y=-23; state.zoom=1.7
    state.selected=E.parse('{:kind :class :id :alpha.deep.parser}'); state.detail_id=E.keyword('alpha.deep.parser')
    state.discussion_context=E.parse('{:context :proposal :proposal-id :discussed}')
    local same,changed=D.reload(state,first,'one',{},'metrics-two')
    assert.is_true(changed); assert.is_true(E.equal(state.focus,same.focus))
    assert.is_true(E.equal(state.selected,same.selected)); assert.is_true(E.equal(state.detail_id,same.detail_id))
    local reduced=first:gsub('{:id :alpha.deep.parser} ','')
    local ancestor,reloaded=D.reload(same,reduced,'two',{},'metrics-three')
    assert.is_true(reloaded); assert.is_true(E.equal(E.parse('[:alpha]'),ancestor.focus))
    assert.is_nil(ancestor.selected); assert.is_nil(ancestor.detail_id)
    local root=D.reload(ancestor,reduced:gsub('{:id :alpha.other} ',''),'three',{},'metrics-four')
    assert.are.equal(0,#root.focus); assert.are.equal(1,#get(root.scene,'classes'))
    for _,s in ipairs{same,ancestor,root} do
      assert.are.same({71,-23,1.7},{s.cam_x,s.cam_y,s.zoom})
      assert.is_true(E.equal(state.discussion_context,s.discussion_context))
    end
  end)
  it('draws public member lines, rules and missing quality as red through adapter commands',function()
    local D=require('uml.application.document'); local IR=require('uml.domain.ir'); local B=require('uml.adapters.class_boxes')
    local scene=D.compile_view(IR.read_document('{:packages [{:label "P" :classes [{:id :a :stereotype :interface :fields ["field"] :ops ["visible" {:text "secret" :private true}]}]}]}'))
    local texts,red,rules={},false,0
    for _,c in ipairs(B.commands(scene)) do if c.kind=='text' then texts[#texts+1]=c.text elseif c.kind=='class' then red=c.red elseif c.kind=='rule' then rules=rules+1 end end
    assert.are.same({'«interface»','a','field','visible'},texts); assert.is_true(red); assert.are.equal(2,rules)
  end)
  it('rejects duplicate class IDs, unknown endpoints and invalid metric ranges',function()
    local IR=require('uml.domain.ir')
    for _,text in ipairs{
      '{:packages [{:label "P" :classes [{:id :a} {:id :a}]}]}',
      '{:packages [{:label "P" :classes [{:id :a}]}] :edges [{:from :a :to :absent}]}',
      '{:packages [{:label "P" :classes [{:id :a :coverage 2}]}]}'
    } do assert.has_error(function() IR.read_document(text) end) end
  end)
end)
