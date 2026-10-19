-- Native Cimgui boundary. All document choices and detail content are pure.
local ffi=require('ffi')
local E=require('uml.edn')
local G=require('uml.domain.geom')
local I=require('uml.application.inspector')
local Detail=require('uml.application.detail')
local S=require('uml.application.strings')
local M={}
function M.new(imgui,callbacks,fonts)
  fonts=fonts or {}
  callbacks=callbacks or {}; local buffer=ffi.new('char[1024]'); local focus=false
  local rename_id,menu_id
  local api={}
  local bounds={}
  local detail_hovered,detail_view_h,detail_scroll,detail_hover=false,0,0,nil
  local empty_rendered=false
  local menu_open=false
  function api.bounds() return bounds end
  function api.name() return ffi.string(buffer) end
  function api.focus_name() focus=true end
  function api.detail_hovered() return detail_hovered end
  function api.detail() return {scroll=detail_scroll,view_h=detail_view_h,hover=detail_hover,rect=bounds['detail-card']} end
  function api.empty_rendered() return empty_rendered end
  function api.menu_open() return menu_open end
  -- Wheel input stays an application transition; the adapter only decides
  -- whether the pointer is over the detail card it drew last frame.
  function api.wheel(state,delta)
    if not detail_hovered then return state end
    local id=G.get(state.selected,'id')
    if not id or not Detail.model(state.scene,id) then return state end
    return I.scroll(state,delta,detail_view_h)
  end
  local function set_name(name)
    name=tostring(name or ''); ffi.fill(buffer,1024,0)
    for i=1,math.min(#name,1023) do buffer[i-1]=name:byte(i) end
    buffer[math.min(#name,1023)]=0
  end
  local function clicked(button)
    if imgui.IsMouseClicked_Bool then return imgui.IsMouseClicked_Bool(button) end
    if imgui.IsMouseClicked then return imgui.IsMouseClicked(button) end
    return false
  end
  local function hovered()
    if imgui.IsItemHovered_Nil then return imgui.IsItemHovered_Nil() end
    if imgui.IsItemHovered then return imgui.IsItemHovered() end
    return false
  end
  local function remember(key)
    local a,b=imgui.GetItemRectMin(),imgui.GetItemRectMax(); bounds[key]={x=(a.x+b.x)/2,y=(a.y+b.y)/2,left=a.x,top=a.y,right=b.x,bottom=b.y}
  end
  function api.draw(state)
    imgui.SetNextWindowPos(imgui.ImVec2_Float(love.graphics.getWidth()-360,12),imgui.ImGuiCond_FirstUseEver)
    imgui.SetNextWindowSize(imgui.ImVec2_Float(348,700),imgui.ImGuiCond_FirstUseEver)
    if imgui.Begin(S.inspector..'###uml-inspector') then
      imgui.TextUnformatted(S.tree_and_proposals)
      for n,c in ipairs(I.commands(state)) do
        if imgui.Selectable_Bool(c.label..'##diagram-'..n,c.active) and callbacks.choose then callbacks.choose(c.id) end
        if c.kind=='proposal' and hovered() and clicked(1) then
          menu_id=c.id
          if imgui.OpenPopup_Str then imgui.OpenPopup_Str('uml-proposal-menu') end
        end
        remember(c.id and 'proposal:'..G.name(c.id) or 'real')
      end
      if imgui.Button(S.new_proposal..'##new-proposal') and callbacks.new_proposal then callbacks.new_proposal() end
      remember('new-proposal')
      if imgui.Button(S.declutter..'##declutter') and callbacks.declutter then callbacks.declutter() end
      remember('declutter')
      menu_open=imgui.BeginPopup('uml-proposal-menu')
      if menu_open then
        if imgui.MenuItem_Bool(S.rename,nil,false,true) then
          local name
          for _,c in ipairs(I.commands(state)) do if E.equal(c.id,menu_id) then name=c.label end end
          rename_id=menu_id; set_name(name); focus=true
        end
        if imgui.MenuItem_Bool(S.delete,nil,false,true) and callbacks.delete then callbacks.delete(menu_id) end
        imgui.EndPopup()
      end
      imgui.Separator(); imgui.TextUnformatted(S.proposal_name)
      if focus then imgui.SetKeyboardFocusHere(); focus=false end
      imgui.InputText('##proposal-name',buffer,1024); remember('proposal-name')
      if rename_id and imgui.Button(S.rename_proposal..'##rename-proposal') then
        if callbacks.rename then callbacks.rename(rename_id,api.name()) end
        rename_id=nil
      end
      if callbacks.name_changed then callbacks.name_changed(api.name()) end
      imgui.TextUnformatted(S.p_hint)
      local id=G.get(state.selected,'id'); local model=id and Detail.model(state.scene,id)
      empty_rendered=false; detail_hovered=false
      if model then
        imgui.Separator()
        local rows=Detail.rows(model)
        local avail=imgui.GetContentRegionAvail()
        detail_view_h=math.max(80,avail.y-24)
        local scroll=Detail.scroll(rows,detail_view_h,state.detail_scroll or 0,0)
        detail_scroll=scroll
        local highlight=Detail.highlight(rows,detail_hover)
        if imgui.BeginChild_Str('detail-card',imgui.ImVec2_Float(0,detail_view_h),
            imgui.ImGuiChildFlags_None,
            imgui.ImGuiWindowFlags_NoScrollbar+imgui.ImGuiWindowFlags_NoScrollWithMouse) then
          detail_hovered=imgui.IsWindowHovered()
          local next_hover=nil
          for n,row in ipairs(rows) do
            imgui.SetCursorPosY(row.y-scroll)
            if row.kind=='group-header' then imgui.TextUnformatted(S.group_crap..'                         '..S.group_mutation)
            elseif row.kind=='col-header' then
              -- Metric names stay English; hovering a header explains it in Chinese.
              for i,col in ipairs(Detail.column_layout()) do
                if i>1 then imgui.SameLine() end
                imgui.TextUnformatted(col.label)
                if hovered() then imgui.BeginTooltip(); imgui.TextUnformatted(S.column_tips[col.id]); imgui.EndTooltip() end
              end
            else
              local text=row.text or ''
              if row.kind=='stats' then
                local values={}; for _,col in ipairs(Detail.column_layout()) do values[#values+1]=row[col.key] or '' end
                text=text..'    '..table.concat(values,'   ')..(row['mut-note'] and '  '..row['mut-note'] or '')
              end
              if row.module or row['op-name'] or row.kind=='rel' then
                if imgui.Selectable_Bool(text..'##detail-'..n,row==highlight) then
                  if row.kind=='rel' then if callbacks.pick then callbacks.pick(row.id) end
                  elseif callbacks.open_source then callbacks.open_source(Detail.member_ident(model,row['op-name'])) end
                end
                if hovered() then next_hover=row.module and E.keyword('module') or row['op-name'] end
                remember(row.module and 'module' or row['op-name'] and 'member:'..row['op-name'] or 'rel:'..G.name(row.id))
              elseif row.kind=='name' and fonts.title then imgui.PushFont(fonts.title); imgui.TextUnformatted(text); imgui.PopFont()
              else imgui.TextUnformatted(text) end
            end
          end
          detail_hover=next_hover
        end
        imgui.EndChild()
        remember('detail-card')
      elseif state.selected then
        local kind=G.name(G.get(state.selected,'kind'))
        if kind=='package' then
          local p=require('uml.engine.hit').package_by_id(state.scene,id)
          if p then imgui.TextUnformatted(G.get(p,'label') or G.name(id)); local count=0; for _,c in ipairs(G.get(state.scene,'classes') or {}) do if E.equal(id,G.get(c,'package')) then count=count+1 end end; imgui.TextUnformatted(S.class_count..count) end
        end
      else
        local message=I.empty_explanation(state)
        if message then imgui.TextUnformatted(message); empty_rendered=true end
      end
      local w,h=love.graphics.getWidth(),love.graphics.getHeight()
      imgui.SetCursorScreenPos(imgui.ImVec2_Float(w-360+16,h-56))
      if imgui.Button(S.regen..'##regen') and callbacks.regen then callbacks.regen() end
    end
    imgui.End()
  end
  return api
end
return M
