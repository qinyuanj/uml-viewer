-- Separate class detail window (640x700) for state.detail_id. Paints the pure
-- detail_view plan: fixed right-aligned columns, 18px rows / 20px title,
-- stripes, swatches, Cov and kill-rate bars, gold hover wash. Clicks go back
-- through callbacks (open_source, pick, close); scroll clamping stays pure.
local G=require('uml.domain.geom')
local T=require('uml.domain.theme')
local Detail=require('uml.application.detail')
local View=require('uml.application.detail_view')
local S=require('uml.application.strings')
local F=require('uml.adapters.fonts')
local okffi,ffi=pcall(require,'ffi')
local M={}
function M.new(imgui,callbacks,fonts)
  fonts=fonts or {}; callbacks=callbacks or {}
  local api,bounds={},{}
  local detail_hovered,detail_view_h,detail_scroll,detail_hover=false,0,0,nil
  function api.bounds() return bounds end
  function api.visible(state) return state~=nil and state.detail_id~=nil end
  function api.detail_hovered() return detail_hovered end
  function api.detail() return {scroll=detail_scroll,view_h=detail_view_h,hover=detail_hover,rect=bounds['detail-card']} end
  -- Wheel input stays an application transition; the adapter only reports
  -- whether the pointer was over the card drawn last frame.
  function api.wheel(state,delta)
    if not detail_hovered or not state.detail_id or not Detail.for_state(state,state.detail_id) then return state end
    return require('uml.application.inspector').scroll(state,delta,detail_view_h)
  end
  local function vec(x,y) return imgui.ImVec2_Float(x,y) end
  local function c4(c,a) return imgui.ImVec4_Float(c[1]/255,c[2]/255,c[3]/255,(a or 255)/255) end
  local function remember(key)
    local a,b=imgui.GetItemRectMin(),imgui.GetItemRectMax(); bounds[key]={x=(a.x+b.x)/2,y=(a.y+b.y)/2,left=a.x,top=a.y,right=b.x,bottom=b.y}
  end
  local function font_of(role) return role=='title' and fonts.title or fonts.code or fonts.body end
  local function size_of(role) return role=='title' and F.sizes.title or F.sizes.code end
  local function measure(font,size,text)
    if font and font.CalcTextSizeA then return font:CalcTextSizeA(size,1e9,0,text,nil,nil).x end
    return imgui.CalcTextSize(text).x*size/F.sizes.body
  end
  local function close() if callbacks.close then callbacks.close() end end
  -- The paint plan, row heights and scroll offsets are view units (the pure
  -- detail layout); display.ui converts them to window px only while painting.
  function api.draw(state,display)
    detail_hovered=false
    if not state.detail_id then return end
    local model=Detail.for_state(state,state.detail_id); if not model then return end
    local ui=display and display.ui or 1
    local rows=Detail.rows(model)
    imgui.SetNextWindowPos(vec(48*ui,80*ui),imgui.ImGuiCond_FirstUseEver)
    imgui.SetNextWindowSize(vec(Detail.width*ui,Detail.height*ui),imgui.ImGuiCond_FirstUseEver)
    imgui.SetNextWindowBgAlpha(1)
    imgui.PushStyleColor_Vec4(imgui.ImGuiCol_WindowBg,c4(T.colors.canvas_bg))
    imgui.PushStyleColor_Vec4(imgui.ImGuiCol_TitleBgActive,c4(T.colors.line))
    local open=okffi and ffi.new('bool[1]',true) or nil
    if imgui.Begin((G.get(model.class,'name') or model.ns or '')..'###uml-detail',open) then
      if imgui.Button(S.close_detail..'##close-detail') then close() end
      remember('close-detail')
      -- Measure inside the child, after its padding and scrollbar allocation.
      -- The 560-unit floor keeps narrower manually resized windows scrollable.
      if imgui.BeginChild_Str('detail-card',vec(0,0),imgui.ImGuiChildFlags_None or 0,(imgui.ImGuiWindowFlags_HorizontalScrollbar or 0)+(imgui.ImGuiWindowFlags_NoScrollWithMouse or 0)) then
        local width=math.max(560*ui,imgui.GetContentRegionAvail().x-16*ui)
        local plan_w=width/ui
        detail_hovered=imgui.IsWindowHovered()==true
        detail_view_h=imgui.GetWindowHeight()/ui
        local scroll=Detail.scroll(rows,detail_view_h,state.detail_scroll or 0,0); detail_scroll=scroll
        local origin=imgui.GetCursorScreenPos(); local list=imgui.GetWindowDrawList()
        local items=View.rows(rows,plan_w)
        bounds.horizontal={maximum=imgui.GetScrollMaxX(),offset=imgui.GetScrollX(),content_width=width}
        -- x in view units; y already in window px.
        local function pt(x,y) return vec(origin.x+x*ui,y) end
        local function text(role,x,top,h,color,s,clip_right)
          local font,size=font_of(role),size_of(role)*ui
          if clip_right then list:PushClipRect(pt(0,top),pt(clip_right,top+h),true) end
          list:AddText_FontPtr(font,size,pt(x,top+(h-size)/2),T.pack(color),tostring(s),nil,0,nil)
          if clip_right then list:PopClipRect() end
        end
        local next_hover
        for n,it in ipairs(items) do
          local top=origin.y+(it.y-scroll)*ui; local h=it.h*ui
          if it.stripe then list:AddRectFilled(pt(0,top),pt(plan_w,top+h),T.pack(T.colors.panel),0,0) end
          if it.clickable then
            imgui.SetCursorScreenPos(pt(0,top))
            local pressed=imgui.InvisibleButton('##detail-'..n,vec(math.max(1,width),h))
            if imgui.IsItemHovered() then
              list:AddRectFilled(pt(0,top),pt(plan_w,top+h),T.pack(T.colors.gold,T.alpha.row_wash),0,0)
              next_hover=it.source.module and require('uml.edn').keyword('module') or it.source['op-name']
            end
            if pressed then
              if it.kind=='rel' then if callbacks.pick then callbacks.pick(it.source.id) end
              elseif callbacks.open_source then callbacks.open_source(Detail.member_ident(model,it.source['op-name'])) end
            end
            remember(it.source.module and 'module' or it.source['op-name'] and 'member:'..it.source['op-name'] or 'rel:'..G.name(it.source.id))
          end
          if it.swatch then list:AddRectFilled(pt(View.pad,top+4*ui),pt(View.pad+View.swatch_w,top+h-4*ui),T.pack(it.swatch),T.radius.swatch*ui,0) end
          if it.text then text(it.font,it.x,top,h,it.color,it.text,it.clip_right) end
          for _,c in ipairs(it.cells) do
            -- Measured at the base size, so the width is already view units.
            local w=measure(font_of('code'),F.sizes.code,c.text)
            local x=c.align=='center' and (c.left+c.right-w)/2 or c.right-w
            text('code',x,top,h,c.color,c.text)
            if c.tip and imgui.IsMouseHoveringRect and imgui.IsMouseHoveringRect(pt(c.left,top),pt(c.right,top+h),true) then
              imgui.BeginTooltip(); imgui.TextUnformatted(c.tip); imgui.EndTooltip()
            end
          end
          local bar=T.widths.bar*ui
          for _,b in ipairs(it.bars) do
            local y=top+h-bar
            list:AddRectFilled(pt(b.left,y),pt(b.right,y+bar),T.pack(T.colors.line),bar/2,0)
            list:AddRectFilled(pt(b.left,y),pt(b.left+(b.right-b.left)*math.max(0,math.min(1,b.fraction)),y+bar),T.pack(b.color),bar/2,0)
          end
        end
        detail_hover=next_hover
      end
      imgui.EndChild(); remember('detail-card')
    end
    imgui.End()
    imgui.PopStyleColor(2)
    if open and not open[0] then close() end
  end
  return api
end
return M
