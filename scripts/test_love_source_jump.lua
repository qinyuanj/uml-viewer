local E=require('uml.edn')
local G=require('uml.domain.geom')
local function save(name,text)
  local f=assert(io.open(os.getenv('UML68_EVIDENCE')..'/'..name,'wb')); f:write(text); f:close()
end
local failed
local function check(condition,message)
  if not condition then failed=failed or message; return false end
  return true
end
local function screenshot(name)
  love.graphics.captureScreenshot(function(data) save(name,data:encode('png'):getString()) end)
end
-- The canvas receives window coordinates; the scene stores world coordinates.
-- Convert through the live camera/zoom, as the navigation driver does.
local function click_rect(state,presses)
  local c=G.get(state.scene,'classes')[1]
  if not c then return nil,'no class in scene' end
  local r=G.get(c,'rect'); local z=require('uml.application.events').zoom_of(state)
  local x=(G.get(r,'x')+6-(state.cam_x or 0))*z
  local y=(G.get(r,'y')+6-(state.cam_y or 0))*z
  love.mousemoved(x,y)
  love.mousepressed(x,y,1,false,presses or 1)
  love.mousereleased(x,y,1)
  return c
end
return function(frame,state,ui)
  if not (ui and ui.source_window and ui.inspector) then failed=failed or 'viewer did not expose integrated callbacks'; return love.event.quit(1) end
  local io=ui.imgui.GetIO()
  local function row(key)
    local b=ui.detail_window.bounds()[key]
    if not b then failed=failed or ('missing inspector row '..key) end
    return b
  end
  local function pin(key) local b=row(key); if b then io:AddMousePosEvent(b.x,b.y) end; return b end
  local function down(key) if pin(key) then io:AddMouseButtonEvent(0,true) end end
  local function up(key) if pin(key) then io:AddMouseButtonEvent(0,false) end end
  if frame==3 then
    -- Actual viewer callback: double-click the namespace container to drill.
    check(click_rect(state,2)~=nil,'could not drill namespace')
  elseif frame==6 then
    -- Actual viewer callback: double-click the leaf to open separate details.
    check(click_rect(state,2)~=nil,'could not open leaf detail')
  elseif frame==8 then pin('module')
  elseif frame==9 then down('module')
  elseif frame==10 then up('module')
  elseif frame==13 then
    local view=ui.source_window.view()
    if check(view~=nil,'module click did not open source') then
      check(view.line==nil,'module must not target a member line')
      check(#view.rows==84,'module must show the whole 83-line file plus trailing row')
      check(view.file:match('/source%.lua$')~=nil,'module must resolve the actual file path')
      save('module.txt',view.file..'\nline=nil\nrows='..#view.rows..'\n')
      screenshot('module.png')
    end
  elseif frame==15 then ui.source_window.close()
  elseif frame==17 then pin('member:M.target')
  elseif frame==18 then down('member:M.target')
  elseif frame==19 then up('member:M.target')
  elseif frame==23 then
    local view=ui.source_window.view()
    if check(view~=nil,'member click did not open source') then
      local viewport=ui.source_window.viewport()
      check(view.line==82,'member must land on the actual line 82')
      check(view.rows[82] and view.rows[82].highlight==true,'target row must be highlighted')
      check(viewport~=nil,'source window must publish viewport evidence')
      if viewport then
        check(viewport.scroll_y>0,'target must require actual scrolling')
        check(viewport.target_visible==true,'target must be visible in the scrolled viewport')
        save('member.txt',view.file..'\nline='..view.line..'\nscroll_y='..viewport.scroll_y..'\ntarget_visible='..tostring(viewport.target_visible)..'\n')
      end
      screenshot('member.png')
    end
  elseif frame==24 then
    local v=ui.source_window.viewport(); local p=v and v.lines and v.lines[82]
    if check(p~=nil,'source selection line bounds missing') then io:AddMousePosEvent(p.x+ui.fonts.code:CalcTextSizeA(13,1e9,0,'  ',nil,nil).x,p.y+p.h/2); io:AddMouseButtonEvent(0,true) end
  elseif frame==26 then
    local v=ui.source_window.viewport(); local p=v and v.lines and v.lines[82]
    if p then io:AddMousePosEvent(p.x+ui.fonts.code:CalcTextSizeA(13,1e9,0,'  function',nil,nil).x,p.y+p.h/2) end
  elseif frame==28 then io:AddMouseButtonEvent(0,false)
  elseif frame==30 then
    local v=ui.source_window.viewport()
    if check(v~=nil,'source window must still be open with a published viewport') then
      check(v.selection=='function','source drag must select actual text without gutter, got '..tostring(v.selection))
    end
    screenshot('selection-chinese-tab.png')
  elseif frame==32 then
    if failed then save('source-jump-fail.txt',failed); love.event.quit(1)
    else save('source-jump-pass.txt','PASS selected leaf -> module full file; member line 82 highlighted and scrolled\n'); love.event.quit(0) end
  end
end
