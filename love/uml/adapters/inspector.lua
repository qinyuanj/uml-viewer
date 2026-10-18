-- Native Cimgui boundary. All document choices and detail content are pure.
local ffi=require('ffi')
local E=require('uml.edn')
local G=require('uml.domain.geom')
local I=require('uml.application.inspector')
local Detail=require('uml.application.detail')
local M={}
function M.fonts(imgui,path)
  path=path or os.getenv('UML_CJK_FONT') or ((os.getenv('WINDIR') or 'C:/Windows')..'/Fonts/msyh.ttc')
  local io=imgui.GetIO(); local font=io.Fonts:AddFontFromFileTTF(path,18,nil,io.Fonts:GetGlyphRangesChineseSimplifiedCommon())
  assert(font~=nil,'CJK font could not be loaded: '..path); io.FontDefault=font; return font
end
function M.new(imgui,callbacks)
  callbacks=callbacks or {}; local buffer=ffi.new('char[1024]'); local focus=false
  local rename_id,menu_id
  local api={}
  local bounds={}
  function api.bounds() return bounds end
  function api.name() return ffi.string(buffer) end
  function api.focus_name() focus=true end
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
    if imgui.Begin('Inspector') then
      imgui.TextUnformatted('Namespace tree / Proposals')
      for n,c in ipairs(I.commands(state)) do
        if imgui.Selectable_Bool(c.label..'##diagram-'..n,c.active) and callbacks.choose then callbacks.choose(c.id) end
        if c.kind=='proposal' and hovered() and clicked(1) then
          menu_id=c.id
          if imgui.OpenPopup_Str then imgui.OpenPopup_Str('uml-proposal-menu') end
        end
        remember(c.id and 'proposal:'..G.name(c.id) or 'real')
      end
      if imgui.Button('New Proposal') and callbacks.new_proposal then callbacks.new_proposal() end
      if imgui.Button('Declutter') and callbacks.declutter then callbacks.declutter() end
      if imgui.BeginPopup('uml-proposal-menu') then
        if imgui.MenuItem_Bool('Rename',nil,false,true) then
          local name
          for _,c in ipairs(I.commands(state)) do if E.equal(c.id,menu_id) then name=c.label end end
          rename_id=menu_id; set_name(name); focus=true
        end
        if imgui.MenuItem_Bool('Delete',nil,false,true) and callbacks.delete then callbacks.delete(menu_id) end
        imgui.EndPopup()
      end
      imgui.Separator(); imgui.TextUnformatted('Proposal name')
      if focus then imgui.SetKeyboardFocusHere(); focus=false end
      imgui.InputText('##proposal-name',buffer,1024); remember('proposal-name')
      if rename_id and imgui.Button('Rename proposal') then
        if callbacks.rename then callbacks.rename(rename_id,api.name()) end
        rename_id=nil
      end
      if callbacks.name_changed then callbacks.name_changed(api.name()) end
      imgui.TextUnformatted('P: namespace tree')
      local id=G.get(state.selected,'id'); local model=id and Detail.model(state.scene,id)
      if model then
        imgui.Separator()
        for n,row in ipairs(Detail.rows(model)) do
          if row.kind=='group-header' then imgui.TextUnformatted('--crap--                         --mutation--')
          elseif row.kind=='col-header' then imgui.TextUnformatted('Crap   CC   Cov   killed   survived   uncovered')
          else
            local text=row.text or ''
            if row.kind=='stats' then
              local values={}; for _,col in ipairs(Detail.column_layout()) do values[#values+1]=row[col.key] or '' end
              text=text..'    '..table.concat(values,'   ')..(row['mut-note'] and '  '..row['mut-note'] or '')
            end
            if row.module or row['op-name'] or row.kind=='rel' then
              if imgui.Selectable_Bool(text..'##detail-'..n,false) then
                if row.kind=='rel' then if callbacks.pick then callbacks.pick(row.id) end
                elseif callbacks.open_source then callbacks.open_source(Detail.member_ident(model,row['op-name'])) end
              end
              remember(row.module and 'module' or row['op-name'] and 'member:'..row['op-name'] or 'rel:'..G.name(row.id))
            else imgui.TextUnformatted(text) end
          end
        end
      elseif state.selected then
        local kind=G.name(G.get(state.selected,'kind'))
        if kind=='package' then
          local p=require('uml.engine.hit').package_by_id(state.scene,id)
          if p then imgui.TextUnformatted(G.get(p,'label') or G.name(id)); local count=0; for _,c in ipairs(G.get(state.scene,'classes') or {}) do if E.equal(id,G.get(c,'package')) then count=count+1 end end; imgui.TextUnformatted('Classes: '..count) end
        end
      end
      local w,h=love.graphics.getWidth(),love.graphics.getHeight()
      imgui.SetCursorScreenPos(imgui.ImVec2_Float(w-360+16,h-56))
      if imgui.Button('Regen') and callbacks.regen then callbacks.regen() end
    end
    imgui.End()
  end
  return api
end
return M
