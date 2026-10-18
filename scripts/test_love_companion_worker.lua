-- Native UI responsiveness while an isolated real Herdr companion initializes.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local root=os.getenv('UML_VIEWER_PROJECT_ROOT')
local stopped=false
return function(frame,state,ui)
  if frame==1 then love.mouse.setPosition(20,20) end
  if frame==4 then love.graphics.captureScreenshot(function(data)
    local f=assert(io.open(root..'/waiting.png','wb')); f:write(data:encode('png'):getString()); f:close()
  end) end
  local status=state.mail_status or ''
  if frame>3 and not stopped and status:find('Companion: ready',1,true) then
    local f=assert(io.open(root..'/worker-ready.txt','wb')); f:write('ready frame='..frame); f:close()
    local queued=assert(io.open(root..'/.uml-viewer/to-viewer.edn','wb'))
    queued:write('{:next-id 2 :queue [{:id 1 :op :context :context :real}]}'); queued:close()
    stopped=true
  elseif stopped and frame>10 then
    assert(G.name(G.get(state.discussion_context,'context'))=='real','context mail was not applied')
    local queue=assert(io.open(root..'/.uml-viewer/to-viewer.edn','rb'))
    local value=E.parse(queue:read('*a')); queue:close()
    assert(#G.get(value,'queue')==0,'context mail was not popped')
    local f=assert(io.open(root..'/worker-ui-pass.txt','wb')); f:write('frames='..frame..' context='..E.encode(state.discussion_context)); f:close()
    love.event.quit(0)
  elseif frame>2 and status:find('ownership retained',1,true) then
    error(status)
  end
end
