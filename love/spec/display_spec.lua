-- Display conversions: world units, DPI-independent view units (what Events
-- consume), logical window px (LOVE/imgui coordinates) and framebuffer px.
local Display=require('uml.domain.display')
describe('display scale',function()
  it('scales layout by the OS DPI the backend has not already applied',function()
    assert.are.equal(1,Display.ui_scale(1,1))
    assert.are.equal(1.25,Display.ui_scale(1.25,1))
    assert.are.equal(1.5,Display.ui_scale(1.5,1))
    -- A backend that already renders at framebuffer scale 2 must not be
    -- magnified again: logical px already cover the 2x pixels.
    assert.are.equal(1,Display.ui_scale(2,2))
    assert.are.equal(1.5,Display.ui_scale(3,2))
    assert.are.equal(1,Display.ui_scale(nil,nil))
    assert.are.equal(1,Display.ui_scale(0,1))
  end)
end)
describe('display sidebar and canvas extent',function()
  it('defaults the inspector to the original 280px times the DPI scale',function()
    local d=Display.new{window_w=1500,window_h=920}
    assert.are.same({1,280,280,1220,1220,920},{d.ui,d.inspector_w,d.sidebar_px,d.canvas_px,d.view_w,d.view_h})
    local hi=Display.new{window_w=2250,window_h=1380,os_scale=1.5}
    assert.are.same({1.5,280,420,1830,1220,920},{hi.ui,hi.inspector_w,hi.sidebar_px,hi.canvas_px,hi.view_w,hi.view_h})
  end)
  it('keeps a saved inspector width in DPI-independent px, clamped to leave a usable canvas',function()
    assert.are.equal(500,Display.new{window_w=1500,window_h=920,inspector_w=500}.inspector_w)
    assert.are.equal(625,Display.new{window_w=1500,window_h=920,os_scale=1.25,inspector_w=500}.sidebar_px)
    assert.are.equal(200,Display.new{window_w=1500,window_h=920,inspector_w=40}.inspector_w)
    -- The canvas keeps at least 240 view units while the window allows it.
    assert.are.equal(1500-240,Display.new{window_w=1500,window_h=920,inspector_w=5000}.inspector_w)
    assert.are.equal(200,Display.new{window_w=300,window_h=200,inspector_w=900}.inspector_w)
    assert.are.equal(0,Display.new{window_w=150,window_h=200}.canvas_px)
  end)
end)
describe('display coordinate conversion',function()
  local Events=require('uml.application.events')
  it('draws world points where pointer hits resolve them, at every DPI and zoom',function()
    for _,os_scale in ipairs{1,1.25,1.5} do for _,zoom in ipairs{.5,1,2.5} do
      local d=Display.new{window_w=1800,window_h=1000,os_scale=os_scale}
      local view={cam_x=40,cam_y=-30,zoom=zoom}
      local x,y=Display.world_to_window(d,view,140,70)
      assert.are.equal((140-40)*zoom*os_scale,x); assert.are.equal((70+30)*zoom*os_scale,y)
      local vx,vy=Display.to_view(d,x,y)
      local wx,wy=Events.world_xy(view,vx,vy)
      assert.is_true(math.abs(wx-140)<1e-9 and math.abs(wy-70)<1e-9)
    end end
  end)
  it('leaves framebuffer px to the backend scale and never compounds it with ui',function()
    local mac=Display.new{window_w=1500,window_h=920,os_scale=2,framebuffer_scale=2}
    assert.are.equal(1,mac.ui); assert.are.equal(2,Display.canvas_scale(mac,2))
    assert.are.same({300,200},{Display.to_framebuffer(mac,150,100)})
    local win=Display.new{window_w=2250,window_h=1380,os_scale=1.5}
    assert.are.equal(3,Display.canvas_scale(win,2))
    assert.are.same({150,100},{Display.to_framebuffer(win,150,100)})
  end)
  it('gives events the canvas extent in view units and stops pointer input at the sidebar',function()
    local d=Display.new{window_w=2250,window_h=1380,os_scale=1.5,inspector_w=300}
    local dims=Display.dims(d)
    assert.are.same({1500,920,1200},{dims.window_w,dims.window_h,dims.view_w})
    assert.is_true(Display.in_canvas(d,1799)); assert.is_false(Display.in_canvas(d,1800)); assert.is_false(Display.in_canvas(d,-1))
  end)
end)
describe('display inspector resize',function()
  it('grabs the inspector left edge within a DPI-scaled grip',function()
    local d=Display.new{window_w=1500,window_h=920}
    assert.is_true(Display.on_inspector_edge(d,1220)); assert.is_true(Display.on_inspector_edge(d,1216))
    assert.is_true(Display.on_inspector_edge(d,1224)); assert.is_false(Display.on_inspector_edge(d,1225))
    local hi=Display.new{window_w=2250,window_h=1380,os_scale=1.5}
    assert.is_true(Display.on_inspector_edge(hi,1830+6)); assert.is_false(Display.on_inspector_edge(hi,1830+7))
  end)
  it('turns a dragged edge position into a DPI-independent clamped width',function()
    local d=Display.new{window_w=1500,window_h=920}
    assert.are.equal(400,Display.drag_inspector(d,1100))
    assert.are.equal(200,Display.drag_inspector(d,1450))
    assert.are.equal(1260,Display.drag_inspector(d,0))
    local hi=Display.new{window_w=2250,window_h=1380,os_scale=1.5}
    assert.are.equal(400,Display.drag_inspector(hi,2250-600))
    local wider=Display.new{window_w=1500,window_h=920,inspector_w=Display.drag_inspector(d,1100)}
    assert.are.same({400,1100,1100},{wider.inspector_w,wider.canvas_px,Display.dims(wider).view_w})
  end)
end)
describe('display window size',function()
  it('opens at the original 1500x920 scaled by DPI, fitted to the desktop',function()
    assert.are.same({1500,920},{Display.window_size(nil,nil,1,3840,2160)})
    assert.are.same({2250,1380},{Display.window_size(nil,nil,1.5,3840,2160)})
    assert.are.same({1920,1080},{Display.window_size(nil,nil,1.5,1920,1080)})
    assert.are.same({1500,920},{Display.window_size(nil,nil,1,nil,nil)})
  end)
  it('restores a saved window size in window px, within the minimum and the desktop',function()
    assert.are.same({1234,777},{Display.window_size(1234,777,1,3840,2160)})
    assert.are.same({640,480},{Display.window_size(100,50,1,3840,2160)})
    assert.are.same({960,720},{Display.window_size(100,50,1.5,3840,2160)})
    assert.are.same({1920,1080},{Display.window_size(5000,4000,1,1920,1080)})
    assert.are.same({1500,920},{Display.window_size('wide',false,1,3840,2160)})
  end)
end)
describe('display stroke widths and font rasters',function()
  local T=require('uml.domain.theme')
  it('multiplies the theme screen-px widths by ui and never by zoom',function()
    local w=Display.widths(Display.new{window_w=1500,window_h=920,os_scale=1.5})
    local got={w.class,w.package,w.selected,w.edge,w.edge_hot,w.edge_violating}
    for i,want in ipairs{3.75,3,5.25,2.4,4.2,3.6} do assert.is_true(math.abs(got[i]-want)<1e-9,i..': '..got[i]) end
    assert.are.equal(2.5,T.widths.class)
    assert.are.same(T.widths,Display.widths(Display.new{window_w=1500,window_h=920}))
  end)
  it('rasterises fonts at device px and draws them at ui-scaled logical size',function()
    local win=Display.new{window_w=1500,window_h=920,os_scale=1.25}
    assert.are.same({22.5,1},{Display.font_raster(win,18),Display.font_global_scale(win)})
    assert.are.equal(22.5,Display.font_size(win,18))
    local mac=Display.new{window_w=1500,window_h=920,os_scale=2,framebuffer_scale=2}
    assert.are.same({36,.5},{Display.font_raster(mac,18),Display.font_global_scale(mac)})
    assert.are.equal(18,Display.font_size(mac,18))
  end)
end)
