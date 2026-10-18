local E=require('uml.edn')
local G=require('uml.domain.geom')
local function save(name,text)
  local f=assert(io.open(os.getenv('UML68_EVIDENCE')..'/'..name,'wb')); f:write(text); f:close()
end
return function(frame,state,ui)
  assert(ui and ui.source_window,'integrated source window required')
  local io=ui.imgui.GetIO()
  local function mouse(key,down)
    local b=assert(ui.inspector.bounds()[key],'missing inspector row '..key)
    io:AddMousePosEvent(b.x,b.y); io:AddMouseButtonEvent(0,down)
  end
  local function screenshot(name)
    love.graphics.captureScreenshot(function(data) save(name,data:encode('png'):getString()) end)
  end
  if frame==3 then
    -- Select through actual viewer hit/selection events, not a source callback.
    local c=assert(G.get(state.scene,'classes')[1]); local r=G.get(c,'rect')
    love.mousepressed(G.get(r,'x')+10,G.get(r,'y')+10,1,false,1)
  elseif frame==6 then
    assert(state.selected,'class must be selected')
    mouse('module',true)
  elseif frame==7 then mouse('module',false)
  elseif frame==10 then
    local view=assert(ui.source_window.view(),'module click did not open source')
    assert(view.line==nil and #view.rows==84)
    save('module.txt',view.file..'\nline=nil\nrows='..#view.rows..'\n')
    screenshot('module.png')
  elseif frame==12 then ui.source_window.close()
  elseif frame==15 then mouse('member:M.target',true)
  elseif frame==16 then mouse('member:M.target',false)
  elseif frame==22 then
    local view=assert(ui.source_window.view(),'member click did not open source')
    local viewport=assert(ui.source_window.viewport())
    assert(view.line==82 and view.rows[82].highlight)
    assert(viewport.scroll_y>0 and viewport.target_visible,'member must be highlighted within scrolled viewport')
    save('member.txt',view.file..'\nline='..view.line..'\nscroll_y='..viewport.scroll_y..'\ntarget_visible='..tostring(viewport.target_visible)..'\n')
    screenshot('member.png')
    save('source-jump-pass.txt','PASS selected class -> module click -> full file; selected member click -> line 82 highlighted and scrolled\n')
  elseif frame==26 then love.event.quit(0) end
end
