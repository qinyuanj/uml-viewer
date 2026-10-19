-- Independent Dear ImGui window in the viewer's LOVE process.
-- Light theme with a line-number gutter, 13px native font, shared palette.
local Source=require('uml.source')
local S=require('uml.application.strings')
local T=require('uml.domain.theme')
local View=require('uml.application.source_view')
local M={}
local function read(path)
  local f,err=io.open(path,'rb'); if not f then return nil,err end
  local text=f:read('*a'); f:close(); return text
end
local function is_file(path)
  local f=io.open(path,'rb')
  if not f then return false end
  local size=f:seek('end'); f:close()
  return size~=nil
end
function M.new(imgui,options)
  options=options or {}
  local fonts=options.fonts or {}
  local service=options.source or Source
  local files={root=options.root,read=options.read or read,is_file=options.is_file or is_file}
  local current,pending,viewport,language,anchor,caret
  local function c4(c) return imgui.ImVec4_Float(c[1]/255,c[2]/255,c[3]/255,1) end
  local api={}
  function api.open(ident)
    local normalized={}
    for key,value in pairs(ident) do normalized[key]=value end
    local E=require('uml.edn')
    if E.kind(normalized.lang)=='keyword' then normalized.lang=normalized.lang.name end
    local ok,result=pcall(service.member_source,normalized,files)
    if not ok then return nil,result end
    if not result then return nil,'Source not found' end
    local rows={}; local first=1
    while true do
      local last=result.body:find('\n',first,true)
      local text=result.body:sub(first,last and last-1 or #result.body):gsub('\r$','')
      rows[#rows+1]={number=#rows+1,text=text,highlight=(#rows+1)==result.line}
      if not last then break end
      first=last+1
    end
    current={title=result.title,file=result.file,line=result.line,rows=rows}
    language=normalized.lang; anchor=nil; caret=nil
    pending=true
    return true
  end
  function api.view() return current end
  function api.viewport() return viewport end
  function api.close() current=nil; pending=nil; viewport=nil end
  function api.draw()
    if not current then return end
    local ffi=require('ffi')
    local visible=ffi.new('bool[1]',true)
    local font=fonts.code or fonts.body
    imgui.SetNextWindowSize(imgui.ImVec2_Float(900,640),imgui.ImGuiCond_FirstUseEver)
    imgui.SetNextWindowBgAlpha(1)
    imgui.PushStyleColor_Vec4(imgui.ImGuiCol_WindowBg,c4(T.colors.source_bg))
    imgui.PushStyleColor_Vec4(imgui.ImGuiCol_ChildBg,c4(T.colors.source_bg))
    imgui.PushStyleColor_Vec4(imgui.ImGuiCol_Text,c4(T.colors.source_ink))
    local expanded=imgui.Begin(current.title..'###uml-source',visible,imgui.ImGuiWindowFlags_HorizontalScrollbar)
    if expanded then
      if imgui.Button(S.close_source..'##close-source') then visible[0]=false end
      imgui.PushStyleColor_Vec4(imgui.ImGuiCol_Text,imgui.ImVec4_Float(90/255,104/255,112/255,1))
      imgui.TextUnformatted(current.file)
      imgui.PopStyleColor(1)
      imgui.BeginChild_Str('source-lines',imgui.ImVec2_Float(0,0),0,imgui.ImGuiWindowFlags_HorizontalScrollbar)
      if font then imgui.PushFont(font) end
      viewport={scroll_y=imgui.GetScrollY(),target_visible=false}
      local gutter_w=44
      imgui.PushStyleVar_Vec2(imgui.ImGuiStyleVar_ItemSpacing,imgui.ImVec2_Float(0,0))
      local line_origins={}
      for _,row in ipairs(current.rows) do
        local display=View.display(row.text)
        local code=display.text
        local p=imgui.GetCursorScreenPos()
        local line_h=imgui.GetTextLineHeight()
        local list=imgui.GetWindowDrawList()
        if row.highlight then
          local width=math.max(imgui.GetContentRegionAvail().x,imgui.CalcTextSize(code).x+gutter_w)
          list:AddRectFilled(imgui.ImVec2_Float(p.x,p.y),imgui.ImVec2_Float(p.x+width,p.y+line_h),T.pack(T.colors.source_highlight),0,0)
        end
        -- Gutter: right-aligned line number on a shaded strip.
        list:AddRectFilled(imgui.ImVec2_Float(p.x,p.y),imgui.ImVec2_Float(p.x+gutter_w,p.y+line_h),T.pack(T.colors.source_gutter_bg),0,0)
        local num=tostring(row.number)
        local num_w=imgui.CalcTextSize(num).x
        list:AddText_FontPtr(font or imgui.GetFont(),imgui.GetFontSize(),imgui.ImVec2_Float(p.x+gutter_w-6-num_w,p.y),T.pack(T.colors.source_gutter),num,nil,0,nil)
        local x=p.x+gutter_w+6
        line_origins[#line_origins+1]={x=x,y=p.y,h=line_h,text=code,to_source=display.to_source}
        if anchor and caret then
          local a,b=View.ordered(anchor,caret)
          if row.number>=a.row and row.number<=b.row then
            local left=row.number==a.row and display.to_display[a.col] or 1
            local right=row.number==b.row and display.to_display[b.col] or #code+1
            local lx=imgui.CalcTextSize(code:sub(1,left-1)).x
            local rx=imgui.CalcTextSize(code:sub(1,right-1)).x
            list:AddRectFilled(imgui.ImVec2_Float(x+lx,p.y),imgui.ImVec2_Float(x+rx,p.y+line_h),T.pack(T.colors.source_selection),0,0)
          end
        end
        for _,run in ipairs(View.tokens(code,language)) do
          local color=T.colors['source_'..run.kind] or T.colors.source_ink
          list:AddText_FontPtr(font or imgui.GetFont(),imgui.GetFontSize(),imgui.ImVec2_Float(x,p.y),T.pack(color),run.text,nil,0,nil)
          x=x+imgui.CalcTextSize(run.text).x
        end
        imgui.InvisibleButton('##source-line-'..row.number,imgui.ImVec2_Float(math.max(imgui.GetContentRegionAvail().x,gutter_w+6+imgui.CalcTextSize(code).x,1),line_h))
        if row.highlight then viewport.target_visible=imgui.IsItemVisible() end
        if pending and row.highlight then imgui.SetScrollHereY(0.25) end
      end
      imgui.PopStyleVar(1)
      local mouse=imgui.GetMousePos()
      local function position()
        for i,p in ipairs(line_origins) do
          if mouse.y>=p.y and mouse.y<p.y+p.h then
            local col=1
            while col<=#p.text do
              local lead=p.text:byte(col); local n=lead<128 and 1 or lead<224 and 2 or lead<240 and 3 or 4
              local l=imgui.CalcTextSize(p.text:sub(1,col-1)).x
              local r=imgui.CalcTextSize(p.text:sub(1,col+n-1)).x
              if mouse.x<p.x+(l+r)/2 then break end
              col=col+n
            end
            return {row=i,col=p.to_source[col]}
          end
        end
      end
      if imgui.IsWindowHovered(imgui.ImGuiHoveredFlags_AllowWhenBlockedByActiveItem) and imgui.IsMouseClicked(0,false) then anchor=position(); caret=anchor end
      if anchor and imgui.IsMouseDown(0) then caret=position() or caret end
      viewport.selection=View.selected(current.rows,anchor,caret)
      viewport.lines=line_origins
      if #viewport.selection>0 and love and love.keyboard and love.keyboard.isDown('lctrl','rctrl') and imgui.IsKeyPressed(imgui.ImGuiKey_C,false) then imgui.SetClipboardText(viewport.selection) end
      if pending and not current.line then imgui.SetScrollY(0) end
      if font then imgui.PopFont() end
      imgui.EndChild()
      pending=nil
    end
    imgui.End()
    imgui.PopStyleColor(3)
    if not visible[0] then api.close() end
  end
  return api
end
return M
