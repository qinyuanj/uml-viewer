-- Native Cimgui boundary for the fixed right sidebar: diagram choices and the
-- selected element summary. All document choices and detail content are pure;
-- the class card lives in the separate detail_window adapter.
local ffi=require('ffi')
local E=require('uml.edn')
local G=require('uml.domain.geom')
local I=require('uml.application.inspector')
local Detail=require('uml.application.detail')
local S=require('uml.application.strings')
local T=require('uml.domain.theme')
local C=require('uml.domain.config')
local Display=require('uml.domain.display')
local M={}
local function pack(c) return T.pack(c) end
function M.new(imgui,callbacks,fonts)
  fonts=fonts or {}
  callbacks=callbacks or {}; local buffer=ffi.new('char[1024]'); local focus=false
  local rename_id,menu_id
  local api={}
  local bounds={}
  local menu_open=false
  function api.bounds() return bounds end
  function api.name() return ffi.string(buffer) end
  function api.focus_name() focus=true end
  function api.empty_rendered() return bounds['empty-help']~=nil end
  function api.menu_open() return menu_open end
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
  local function inked(text,color)
    color=color or T.colors.ink
    imgui.PushStyleColor_Vec4(imgui.ImGuiCol_Text,imgui.ImVec4_Float(color[1]/255,color[2]/255,color[3]/255,1))
    imgui.TextUnformatted(tostring(text or ''))
    imgui.PopStyleColor(1)
  end
  -- Selected class summary: identity, quality lines, then member lines. The
  -- full six-column metrics card opens in the detail window, not here.
  local function class_summary(model)
    local c=model.class
    inked(G.get(c,'name'))
    local package=G.get(c,'package')
    inked(S.package..(G.get(G.get(model,'package'),'label') or (package and G.name(package)) or 'foreign'),T.colors.muted)
    if G.get(c,'level')~=nil then inked(S.level..G.get(c,'level'),T.colors.muted) end
    if G.get(c,'stereotype') then inked('«'..G.name(G.get(c,'stereotype'))..'»',T.colors.muted) end
    local crap=G.get(c,'crap')
    if type(crap)=='number' then inked(S.crap_single:format(string.format('%.1f',crap)),T.colors.gold)
    elseif crap and G.get(crap,'mu')~=nil then
      inked(S.crap_line:format(string.format('%.1f',G.get(crap,'mu')),string.format('%.1f',G.get(crap,'max') or G.get(crap,'mu')),string.format('%.1f',G.get(crap,'sigma') or 0)),T.colors.gold)
    end
    local killed,survived,uncovered=G.get(c,'killed'),G.get(c,'survived'),G.get(c,'uncovered')
    if killed~=nil or survived~=nil then
      local line=S.package_mutation:format(killed or 0,survived or 0)
      if uncovered and uncovered>0 then line=line..S.package_uncovered:format(uncovered) end
      inked(line,T.colors.muted)
    end
    for _,line in ipairs(G.get(c,'lines') or {}) do
      local kind=G.name(G.get(line,'kind'))
      if kind=='field' or kind=='op' then inked(G.get(line,'text')) end
    end
    if G.get(crap,'mu')==nil and type(crap)~='number' then inked(S.no_data,T.colors.band.red) end
  end
  local function package_summary(state,id)
    local p=require('uml.engine.hit').package_by_id(state.scene,id); if not p then return end
    inked(G.get(p,'label') or G.name(id))
    local crap=G.get(p,'crap')
    if crap and G.get(crap,'mu')~=nil then
      inked(S.package_crap:format(string.format('%.1f',G.get(crap,'mu')),string.format('%.1f',G.get(crap,'max') or G.get(crap,'mu')),string.format('%.1f',G.get(crap,'sigma') or 0)),T.colors.gold)
    end
    local killed,survived,uncovered=G.get(p,'killed'),G.get(p,'survived'),G.get(p,'uncovered')
    if killed~=nil or survived~=nil then
      local line=S.package_mutation:format(killed or 0,survived or 0)
      if uncovered and uncovered>0 then line=line..S.package_uncovered:format(uncovered) end
      inked(line,T.colors.muted)
    end
    local count=0; for _,c in ipairs(G.get(state.scene,'classes') or {}) do if E.equal(id,G.get(c,'package')) then count=count+1 end end
    inked(S.class_count..count,T.colors.muted)
    if crap==nil then inked(S.no_data,T.colors.band.red) end
  end
  -- display (domain.display) fixes the sidebar: window px left edge, width
  -- sidebar_px and the full window height; its style spacing scales by ui.
  local sidebar_px
  function api.width() return sidebar_px or I.width end
  function api.draw(state,display)
    display=display or Display.new{window_w=love.graphics.getWidth(),window_h=love.graphics.getHeight()}
    local ui=display.ui; sidebar_px=display.sidebar_px
    imgui.SetNextWindowPos(imgui.ImVec2_Float(display.canvas_px,0),imgui.ImGuiCond_Always)
    imgui.SetNextWindowSize(imgui.ImVec2_Float(display.sidebar_px,display.window_h),imgui.ImGuiCond_Always)
    imgui.SetNextWindowBgAlpha(1)
    local bg=T.colors.sidebar
    imgui.PushStyleColor_Vec4(imgui.ImGuiCol_WindowBg,imgui.ImVec4_Float(bg[1]/255,bg[2]/255,bg[3]/255,1))
    imgui.PushStyleVar_Vec2(imgui.ImGuiStyleVar_WindowPadding,imgui.ImVec2_Float(16*ui,16*ui))
    imgui.PushStyleVar_Vec2(imgui.ImGuiStyleVar_ItemSpacing,imgui.ImVec2_Float(8*ui,6*ui))
    imgui.PushStyleVar_Float(imgui.ImGuiStyleVar_FrameRounding,T.radius.button*ui)
    local flags=imgui.ImGuiWindowFlags_NoTitleBar+imgui.ImGuiWindowFlags_NoMove
      +imgui.ImGuiWindowFlags_NoResize+imgui.ImGuiWindowFlags_NoCollapse
      +imgui.ImGuiWindowFlags_NoBringToFrontOnFocus
    if imgui.Begin('###uml-inspector',nil,flags) then
      remember('inspector-window')
      if fonts.title then imgui.PushFont(fonts.title) end
      inked(S.inspector,T.colors.gold)
      if fonts.title then imgui.PopFont() end
      for n,c in ipairs(I.commands(state)) do
        if c.kind=='proposal' and (n==2 or #I.commands(state)==1) and n==2 then inked(S.proposals_label,T.colors.muted) end
        if imgui.Selectable_Bool(c.label..'##diagram-'..n,c.active) and callbacks.choose then callbacks.choose(c.id) end
        if c.kind=='proposal' and hovered() and clicked(1) then
          menu_id=c.id
          if imgui.OpenPopup_Str then imgui.OpenPopup_Str('uml-proposal-menu') end
        end
        remember(c.id and 'proposal:'..G.name(c.id) or 'real')
      end
      if imgui.Button(S.new_proposal..'##new-proposal',imgui.ImVec2_Float(-1,0)) and callbacks.new_proposal then callbacks.new_proposal() end
      remember('new-proposal')
      if imgui.Button(S.declutter..'##declutter',imgui.ImVec2_Float(-1,0)) and callbacks.declutter then callbacks.declutter() end
      remember('declutter')
      menu_open=imgui.BeginPopup('uml-proposal-menu')
      if menu_open then
        if imgui.MenuItem_Bool(S.rename,nil,false,true) then
          local name
          for _,c in ipairs(I.commands(state)) do if E.equal(c.id,menu_id) then name=c.label end end
          rename_id=menu_id; set_name(name); focus=true
        end
        remember('rename-menu')
        if imgui.MenuItem_Bool(S.delete,nil,false,true) and callbacks.delete then callbacks.delete(menu_id) end
        imgui.EndPopup()
      end
      if rename_id then
        imgui.Separator(); inked(S.proposal_name,T.colors.muted)
        if focus then imgui.SetKeyboardFocusHere(); focus=false end
        imgui.InputText('##proposal-name',buffer,1024); remember('proposal-name')
        if imgui.Button(S.rename_proposal..'##rename-proposal') then
          if callbacks.rename then callbacks.rename(rename_id,api.name()) end
          rename_id=nil
        end
        if imgui.Button(S.close_source..'##cancel-rename') then rename_id=nil end
      end
      if callbacks.name_changed then callbacks.name_changed(api.name()) end
      inked(S.p_hint,T.colors.muted)
      imgui.Separator()
      local id=G.get(state.selected,'id')
      if imgui.BeginChild_Str('summary-card',imgui.ImVec2_Float(0,-40*ui),imgui.ImGuiChildFlags_None,0) then
        local kind=G.name(G.get(state.selected,'kind'))
        local model=id and Detail.model(state.scene,id)
        if model then class_summary(model)
        elseif kind=='package' and id then package_summary(state,id)
        else
          local message=I.empty_explanation(state)
          if message then inked(message,T.colors.muted); remember('empty-help')
          elseif state.selected then inked(S.select_class,T.colors.muted); remember('empty-help') end
        end
      end
      imgui.EndChild()
      remember('summary-card')
      if imgui.Button(S.regen..'##regen',imgui.ImVec2_Float(-1,0)) and callbacks.regen then callbacks.regen() end
      remember('regen')
    end
    imgui.End()
    imgui.PopStyleColor(1)
    imgui.PopStyleVar(3)
  end
  return api
end
return M
