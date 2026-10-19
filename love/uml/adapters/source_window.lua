-- Independent Dear ImGui window in the viewer's LOVE process.
local Source=require('uml.source')
local S=require('uml.application.strings')
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
  local service=options.source or Source
  local files={root=options.root,read=options.read or read,is_file=options.is_file or is_file}
  local current,pending,viewport
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
    imgui.SetNextWindowSize(imgui.ImVec2_Float(900,640),imgui.ImGuiCond_FirstUseEver)
    local expanded=imgui.Begin(current.title..'###uml-source',visible,imgui.ImGuiWindowFlags_HorizontalScrollbar)
    if expanded then
      if imgui.Button(S.close_source..'##close-source') then visible[0]=false end
      imgui.TextUnformatted(current.file)
      imgui.BeginChild_Str('source-lines',imgui.ImVec2_Float(0,0),0,imgui.ImGuiWindowFlags_HorizontalScrollbar)
      viewport={scroll_y=imgui.GetScrollY(),target_visible=false}
      for _,row in ipairs(current.rows) do
        local label=string.format('%5d  %s',row.number,row.text:gsub('\t','  '))
        -- TextUnformatted preserves literal ## and %, while selection supplies highlight.
        if row.highlight then
          local p=imgui.GetCursorScreenPos()
          local width=math.max(imgui.GetContentRegionAvail().x,imgui.CalcTextSize(label).x)
          imgui.GetWindowDrawList():AddRectFilled(p,imgui.ImVec2_Float(p.x+width,p.y+imgui.GetTextLineHeight()),0xff455a65,0,0)
        end
        imgui.TextUnformatted(label)
        if row.highlight then viewport.target_visible=imgui.IsItemVisible() end
        if pending and row.highlight then imgui.SetScrollHereY(0.25) end
      end
      if pending and not current.line then imgui.SetScrollY(0) end
      imgui.EndChild()
      pending=nil
    end
    imgui.End()
    if not visible[0] then api.close() end
  end
  return api
end
return M
