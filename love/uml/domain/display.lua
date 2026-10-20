-- Display spaces, outermost first:
--   world       scene geometry (layout units).
--   view        DPI-independent canvas units; application events consume these.
--   window      logical window px: the LOVE/imgui coordinate space.
--   framebuffer device px; the backend multiplies window px by its own
--               framebuffer scale (imgui DisplayFramebufferScale).
-- ui is the DPI factor the viewer itself applies between view and window: the
-- OS DPI scale divided by what the backend already renders, so nothing is
-- magnified twice.
local T=require('uml.domain.theme')
local M={}
local function positive(x) return type(x)=='number' and x>0 and x or nil end
function M.ui_scale(os_scale,framebuffer_scale)
  local fb=positive(framebuffer_scale) or 1
  local os=positive(os_scale) or fb
  return math.max(os,fb)/fb
end
-- Inspector widths are DPI-independent px (view units): the original 280px
-- sidebar, never narrower than 200, leaving the canvas at least 240 units.
M.inspector={default=280,min=200}
M.canvas_min=240
function M.clamp_inspector(width,window_w,ui)
  ui=positive(ui) or 1
  local w=positive(width) or M.inspector.default
  local most=math.max(M.inspector.min,(window_w or 0)/ui-M.canvas_min)
  return math.max(M.inspector.min,math.min(most,w))
end
-- One record per frame: window px from the backend plus the viewer's ui scale.
function M.new(opts)
  local fb=positive(opts.framebuffer_scale) or 1
  local ui=M.ui_scale(opts.os_scale,fb)
  local window_w,window_h=opts.window_w or 0,opts.window_h or 0
  local inspector_w=M.clamp_inspector(opts.inspector_w,window_w,ui)
  local sidebar_px=inspector_w*ui
  local canvas_px=math.max(0,window_w-sidebar_px)
  return {ui=ui,fb=fb,window_w=window_w,window_h=window_h,inspector_w=inspector_w,
    sidebar_px=sidebar_px,canvas_px=canvas_px,view_w=canvas_px/ui,view_h=window_h/ui}
end
-- world -> window: drawing multiplies the camera zoom by ui exactly once.
function M.canvas_scale(d,zoom) return (zoom or 1)*d.ui end
function M.world_to_window(d,view,wx,wy)
  local s=M.canvas_scale(d,view.zoom)
  return (wx-(view.cam_x or 0))*s,(wy-(view.cam_y or 0))*s
end
-- window -> view: pointer input enters application events in view units.
function M.to_view(d,x,y) return x/d.ui,y/d.ui end
-- window -> framebuffer: the backend's own scale, never multiplied by ui.
function M.to_framebuffer(d,x,y) return x*d.fb,y*d.fb end
-- Event extents in view units (the shape events expect from dims).
function M.dims(d) return {window_w=d.window_w/d.ui,window_h=d.view_h,view_w=d.view_w} end
function M.in_canvas(d,x) return x>=0 and x<d.canvas_px end
-- Window sizes are window px. The default is the original 1500x920 view-unit
-- window; saved sizes restore as-is. Both stay within 640x480 view units and
-- the desktop.
M.window={w=1500,h=920}
M.window_min={w=640,h=480}
function M.window_size(saved_w,saved_h,ui,desktop_w,desktop_h)
  ui=positive(ui) or 1
  local w,h=positive(saved_w),positive(saved_h)
  if not (w and h) then w,h=M.window.w*ui,M.window.h*ui end
  w,h=math.max(M.window_min.w*ui,w),math.max(M.window_min.h*ui,h)
  return math.floor(math.min(w,positive(desktop_w) or w)+.5),math.floor(math.min(h,positive(desktop_h) or h)+.5)
end
-- The sidebar's left edge is draggable within 4 view units either side.
M.edge_grip=4
function M.on_inspector_edge(d,x) return math.abs(x-d.canvas_px)<=M.edge_grip*d.ui end
function M.drag_inspector(d,x) return M.clamp_inspector((d.window_w-x)/d.ui,d.window_w,d.ui) end
-- Screen-px strokes: the theme baseline times ui (zoom never enters).
function M.widths(d) return T.dpi(d.ui) end
-- Fonts rasterise at device px; imgui's global font scale undoes the backend
-- framebuffer scale so widget text measures size*ui window px.
function M.font_size(d,size) return size*d.ui end
function M.font_raster(d,size) return size*d.ui*d.fb end
function M.font_global_scale(d) return 1/d.fb end
return M
