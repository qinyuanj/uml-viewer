-- Native Cimgui boundary. All document choices and detail content are pure.
local ffi=require('ffi')
local E=require('uml.edn')
local G=require('uml.domain.geom')
local I=require('uml.application.inspector')
local Detail=require('uml.application.detail')
local M={}
function M.fonts(imgui,path)
  path=path or os.getenv('UML_CJK_FONT') or ((os.getenv('WINDIR') or 'C:/Windows')..'/Fonts/msyh.ttc')
  local io=imgui.GetIO(); local font=io.Fonts:AddFontFromFileTTF(path,18,nil,io.Fonts:GetGlyphRangesChineseFull())
  assert(font~=nil,'CJK font could not be loaded: '..path); return font
end
function M.new(imgui,callbacks)
  callbacks=callbacks or {}; local buffer=ffi.new('char[1024]'); local focus=false
  local api={}
  function api.name() return ffi.string(buffer) end
  function api.focus_name() focus=true end
  function api.draw(state)
    imgui.SetNextWindowPos(imgui.ImVec2(love.graphics.getWidth()-360,12),imgui.ImGuiCond_FirstUseEver)
    imgui.SetNextWindowSize(imgui.ImVec2(348,700),imgui.ImGuiCond_FirstUseEver)
    if imgui.Begin('Inspector') then
      imgui.TextUnformatted('Namespace tree / Proposals')
      for n,c in ipairs(I.commands(state)) do
        if imgui.Selectable_Bool(c.label..'##diagram-'..n,c.active) and callbacks.choose then callbacks.choose(c.id) end
      end
      imgui.Separator(); imgui.TextUnformatted('Proposal name')
      if focus then imgui.SetKeyboardFocusHere(); focus=false end
      imgui.InputText('##proposal-name',buffer,1024)
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
    end
    imgui.End()
  end
  return api
end
return M
