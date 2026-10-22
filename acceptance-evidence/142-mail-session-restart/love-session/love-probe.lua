-- Throwaway #142 probe: make the real LOVE viewer write session.edn through
-- its own :quit-for-restart path, after every restored dimension changed.
-- Selection and class card come from real pointer clicks, so the file carries
-- the ids LOVE's own scene uses, not hand-written ones.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local Display=require('uml.domain.display')
local Atomic=require('uml.adapters.atomic_file')
local root=os.getenv('UML_VIEWER_PROJECT_ROOT')
local function queue(t) Atomic.write(root..'/.uml-viewer/to-viewer.edn',t) end
local target
return function(frame,state,hooks)
  local d=hooks.display()
  if frame==5 then
    queue('{:next-id 2 :queue [{:id 1 :op :context :context :proposal :proposal-id :plan :name "中文方案"}]}')
  elseif frame==8 then
    for _,c in ipairs(G.get(state.scene,'classes') or {}) do
      local r=G.get(c,'rect')
      if r and not G.get(c,'dummy?') then
        local x,y=Display.world_to_window(d,state,G.get(r,'x')+6,G.get(r,'y')+6)
        if Display.in_canvas(d,x) and y<d.window_h then target={x=x,y=y}; break end
      end
    end
    assert(target,'no class on the canvas')
    love.mousemoved(target.x,target.y,0,0)
  elseif frame==10 then love.mousepressed(target.x,target.y,1,false,1)
  elseif frame==11 then love.mousereleased(target.x,target.y,1,false,1)
  elseif frame==13 then love.mousepressed(target.x,target.y,1,false,2)
  elseif frame==14 then love.mousereleased(target.x,target.y,1,false)
  elseif frame==16 then
    assert(state.selected and state.detail_id,'click did not select and open the card')
    love.mousemoved(d.canvas_px,200,0,0)
  elseif frame==18 then love.mousepressed(d.canvas_px,200,1,false,1)
  elseif frame==19 then love.mousemoved(d.canvas_px-60,200,-60,0)
  elseif frame==20 then love.mousereleased(d.canvas_px-60,200,1,false,1)
  elseif frame==22 then love.window.updateMode(1100,700,{resizable=true,highdpi=true})
  elseif frame==40 then
    assert(d.inspector_w>280,'inspector drag did not widen the sidebar: '..tostring(d.inspector_w))
    state.zoom=1.7; state.cam_x=33; state.cam_y=21
  elseif frame==60 then
    assert(E.encode(state.discussion_context):find(':plan',1,true),'context mail not applied')
    queue('{:next-id 3 :queue [{:id 2 :op :quit-for-restart}]}')
  end
end
