-- Runs only in a newly launched native viewer with a temporary document.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local I=require('uml.application.inspector')
local evidence=os.getenv('UML_VIEWER_EVIDENCE')
local failed
local function check(condition,message)
  if not condition then failed=failed or message; return false end
  return true
end
local function save(name,text)
  local f=assert(io.open(evidence..'/'..name,'wb')); f:write(text); f:close()
end
local function screenshot(name)
  love.graphics.captureScreenshot(function(data) save(name,data:encode('png'):getString()) end)
end
local proposal_count
return function(frame,state,ui)
  if not (ui and ui.inspector) then failed=failed or 'viewer did not expose the inspector adapter'; love.event.quit(1); return end
  local ins=ui.inspector; local io=ui.imgui.GetIO()
  local function bounds(key) return ins.bounds()[key] end
  local function pin(key) local b=bounds(key); if b then io:AddMousePosEvent(b.x,b.y) else failed=failed or ('missing inspector row '..key) end; return b end
  if frame==2 then
    check(ins.empty_rendered()==true,'empty-inspector explanation was not drawn')
    screenshot('inspector-empty.png')
  elseif frame==3 then
    state.selected=G.map{kind=E.keyword('class'),id=E.keyword('a')}
    state.detail_scroll=0
  elseif frame==5 then
    local d=ins.detail(); check(d.rect~=nil,'detail card bounds missing')
    if d.rect then io:AddMousePosEvent((d.rect.left+d.rect.right)/2,(d.rect.top+d.rect.bottom)/2) end
  elseif frame==6 then
    check(ins.detail_hovered()==true,'pointer over the detail card was not detected')
    love.wheelmoved(0,-4)
  elseif frame==7 then
    check(state.detail_scroll==192,'four wheel notches must scroll four 48px steps')
    love.wheelmoved(0,-1000)
  elseif frame==8 then
    local d=ins.detail()
    check(d.scroll==state.detail_scroll,'reported scroll must match application state')
    check(d.scroll>192 and d.scroll<10000,'overscroll must clamp to the content end')
    love.wheelmoved(0,1000)
  elseif frame==9 then
    check(state.detail_scroll==0,'underscroll must clamp to zero')
    love.wheelmoved(0,-3)
  elseif frame==10 then
    check(state.detail_scroll==144,'three wheel notches must restore 144px')
    pin('member:op8')
  elseif frame==11 then
    check(ins.detail().hover=='op8','hovered operation must be published from the drawn row')
    screenshot('inspector.png')
  elseif frame==12 then
    proposal_count=#I.commands(state)
    pin('new-proposal')
  elseif frame==13 then
    io:AddMouseButtonEvent(0,true)
  elseif frame==14 then
    io:AddMouseButtonEvent(0,false)
  elseif frame==16 then
    check(#I.commands(state)==proposal_count+1,'New Proposal must add one row')
    pin('proposal:plan')
  elseif frame==17 then
    io:AddMouseButtonEvent(1,true)
  elseif frame==18 then
    io:AddMouseButtonEvent(1,false)
  elseif frame==20 then
    check(ui.inspector.menu_open()==true,'proposal Rename/Delete menu did not open')
  elseif frame==22 then
    if failed then save('inspector-detail-fail.txt',failed); love.event.quit(1)
    else
      save('inspector-pass.txt','PASS empty-inspector copy, clamped detail scrolling, hovered op, New Proposal row and Rename/Delete menu\n')
      love.event.quit(0)
    end
  end
end
