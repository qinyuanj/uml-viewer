-- Canvas navigation (#85): input event sequences through Events, observed as
-- view state and the draw commands the canvas renders.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local D=require('uml.application.document')
local Events=require('uml.application.events')
local Hit=require('uml.engine.hit')
local g,k=G.get,E.keyword
local function near(a,b,message) assert.is_true(math.abs(a-b)<1e-9,(message or '')..' expected '..tostring(a)..' actual '..tostring(b)) end
local function tree()
  return D.new(E.parse('{:hierarchical true :title "Tree" :classes [{:id :source} {:id :source.lua} {:id :source.lua.parser} {:id :other}] :edges []}'),'one')
end
local dims={window_w=1500,window_h=900,view_w=1220}
local function with(extra) local d={}; for key,v in pairs(dims) do d[key]=v end; for key,v in pairs(extra or {}) do d[key]=v end; return d end
describe('canvas navigation',function()
  it('zooms with control+wheel around the cursor, keeping the world point under it',function()
    local s=tree(); s.cam_x=30; s.cam_y=40; s.zoom=1
    local wx,wy=Events.world_xy(s,400,300)
    local a=Events.on_wheel(s,0,1,with{control=true,pointer={400,300}})
    near(1.1,a.zoom,'one notch in')
    local ax,ay=Events.world_xy(a,400,300); near(wx,ax,'anchor x'); near(wy,ay,'anchor y')
    local b=Events.on_wheel(a,0,-1,with{control=true,pointer={400,300}})
    near(1,b.zoom,'one notch out'); near(30,b.cam_x); near(40,b.cam_y)
    for _=1,100 do a=Events.on_wheel(a,0,1,with{control=true,pointer={10,10}}) end; near(10,a.zoom,'clamped high')
    for _=1,100 do a=Events.on_wheel(a,0,-1,with{control=true,pointer={10,10}}) end; near(.1,a.zoom,'clamped low')
  end)
  it('pans with the plain wheel, sideways with shift or a horizontal wheel',function()
    local s=tree(); s.scene=G.map{size=G.map{w=4000,h=4000}}
    local down=Events.on_wheel(s,0,-1,with{}); near(48,down.cam_y); near(0,down.cam_x or 0); assert.is_nil(down.zoom)
    local side=Events.on_wheel(s,0,-1,with{shift=true}); near(48,side.cam_x); near(0,side.cam_y or 0)
    local wheel=Events.on_wheel(s,-2,0,with{}); near(96,wheel.cam_x)
    s.zoom=2; near(24,Events.on_wheel(s,0,-1,with{}).cam_y,'pan distance shrinks with zoom')
  end)
  it('zooms with control+plus and control+minus around the cursor, else the canvas centre',function()
    local s=tree(); s.cam_x=5; s.cam_y=7
    local wx,wy=Events.world_xy(s,200,100)
    local a=Events.on_key(s,'=',with{control=true,pointer={200,100}}); near(1.1,a.zoom)
    local ax,ay=Events.world_xy(a,200,100); near(wx,ax); near(wy,ay)
    local b=Events.on_key(a,'-',with{control=true,pointer={200,100}}); near(1,b.zoom); near(5,b.cam_x); near(7,b.cam_y)
    local cx,cy=Events.world_xy(s,610,450)
    local centred=Events.on_key(s,'kp+',with{control=true}); local x,y=Events.world_xy(centred,610,450); near(cx,x); near(cy,y)
  end)
  it('anchors on the last canvas pointer and forgets it, with the hover, when the pointer leaves',function()
    local s=Events.on_move(tree(),200,100)
    local wx,wy=Events.world_xy(s,200,100)
    local a=Events.on_wheel(s,0,1,with{control=true}); local x,y=Events.world_xy(a,200,100); near(wx,x); near(wy,y)
    local left=Events.on_leave(Events.on_move(tree(),80,110)); assert.is_nil(left.pointer); assert.is_nil(left.hover)
    local cx,cy=Events.world_xy(left,610,450)
    local b=Events.on_wheel(left,0,1,with{control=true}); x,y=Events.world_xy(b,610,450); near(cx,x); near(cy,y)
  end)
  it('fits the whole diagram into the canvas left of the sidebar with control+0',function()
    local s=tree(); s.zoom=3; s.cam_x=999; s.cam_y=-5
    s.scene=G.map{size=G.map{w=2440,h=900,['min-x']=0,['min-y']=0}}
    local f=Events.on_key(s,'0',with{control=true})
    near(.5,f.zoom,'width-bound: 1220/2440')
    -- Centred: the diagram middle sits at the canvas middle.
    local mx,my=Events.world_xy(f,610,450); near(1220,mx); near(450,my)
    s.scene=G.map{size=G.map{w=300,h=200,['min-x']=-100,['min-y']=0}}
    local small=Events.fit(s,dims); near(1,small.zoom,'small diagrams are not blown up past 100%')
    local sx,sy=Events.world_xy(small,610,450); near(100,sx); near(100,sy)
  end)
  it('fits after a drill, back or proposal switch once the canvas size is known, never on generic rebuilds',function()
    local s=tree(); s.zoom=4; s.cam_x=50
    local entered=Events.drill(s,k('source')); assert.is_true(entered.fit_pending)
    local settled=Events.settle(entered,dims); assert.is_nil(settled.fit_pending)
    local size=g(entered.scene,'size'); local w=g(size,'w')-(g(size,'min-x') or 0)
    near(math.min(1,1220/w,900/(g(size,'h')-(g(size,'min-y') or 0))),settled.zoom)
    assert.is_true(Events.back(settled).fit_pending)
    assert.is_true(Events.show_proposal(settled,nil).fit_pending)
    settled.zoom=2; settled.cam_x=11
    settled.selected=G.map{kind=k('class'),id=k('source.lua')}
    local cleared=Events.cycle_declutter(settled); assert.is_nil(cleared.fit_pending); near(2,cleared.zoom); near(11,cleared.cam_x)
    assert.is_true(E.equal(settled.selected,cleared.selected),'generic rebuild retains valid selection')
    assert.are.equal(settled,Events.settle(settled,dims),'nothing pending leaves the view alone')
  end)
  it('fits a newly displayed diagram but retains the view on same-diagram display and reloads (ADR 0002)',function()
    local text='{:hierarchical true :title "Tree" :classes [{:id :source} {:id :source.lua} {:id :other}] :edges []}'
    local boundary={resolve=function(p) return p end,canonical=function(p) return p end,
      load=function(p) local s=D.new(E.parse(text),'m1'); s.path=p; return s end}
    local function display(s,path,id) return D.apply_mail(s,G.map{id=id,op=k('display'),path=path},boundary) end
    local current=boundary.load('a.edn'); current.project_root='.'; current.zoom=2.5; current.cam_x=33; current.cam_y=44
    current.selected=G.map{kind=k('class'),id=k('source')}
    local same=display(current,'a.edn',1)
    assert.is_nil(same.fit_pending); near(2.5,same.zoom); near(33,same.cam_x); near(44,same.cam_y)
    assert.is_true(E.equal(k('source'),g(same.selected,'id')))
    local other=display(current,'b.edn',2); assert.is_true(other.fit_pending)
    local waiting=D.waiting_state('a.edn',0); waiting.project_root='.'
    assert.is_true(display(waiting,'a.edn',1).fit_pending,'first display after a fresh launch is a new load')
    local reloaded=D.reload(same,text,'m2',nil,'stamp'); assert.is_nil(reloaded.fit_pending); near(2.5,reloaded.zoom)
    local Session=require('uml.application.session')
    local saved=Session.snapshot(current); local fresh=boundary.load('a.edn'); fresh.fit_pending=true
    local restored=Session.restore(fresh,saved)
    assert.is_nil(restored.fit_pending,'restart must not refit a saved view')
    near(2.5,restored.zoom); near(33,restored.cam_x); near(44,restored.cam_y)
    assert.is_true(E.equal(current.selected,restored.selected))
  end)
  local function class_point(s,id)
    local r=g(Hit.class_by_id(s.scene,k(id)),'rect'); local z=Events.zoom_of(s)
    return (g(r,'x')+5-(s.cam_x or 0))*z,(g(r,'y')+5-(s.cam_y or 0))*z
  end
  it('pans by dragging blank canvas, keeping the selection, and clicks blank to clear it',function()
    local s=tree(); s.zoom=2; s.cam_x=10; s.cam_y=20; s.selected=G.map{kind=k('class'),id=k('source')}
    local a=Events.on_left_press(s,1000,800); assert.is_not_nil(a.drag)
    local b=Events.on_drag(a,1001,801); near(10,b.cam_x,'below the drag threshold nothing pans')
    local c=Events.on_drag(b,960,700); near(30,c.cam_x); near(70,c.cam_y)
    local d=Events.on_release(c,960,700); assert.is_nil(d.drag)
    assert.is_true(E.equal(k('source'),g(d.selected,'id')),'a pan keeps the selection')
    near(30,d.cam_x)
    local click=Events.on_release(Events.on_left_press(s,1000,800),1000,800)
    assert.is_nil(click.selected,'a blank click clears the selection'); near(10,click.cam_x)
  end)
  it('does nothing when a drag starts on a class, which a press still selects',function()
    local s=tree(); local x,y=class_point(s,'source')
    local a=Events.on_left_press(s,x,y); assert.is_nil(a.drag)
    assert.is_true(E.equal(k('source'),g(a.selected,'id')))
    local b=Events.on_release(Events.on_drag(a,x+200,y+200),x+200,y+200)
    near(0,b.cam_x or 0); near(0,b.cam_y or 0); assert.is_true(E.equal(k('source'),g(b.selected,'id')))
  end)
end)
