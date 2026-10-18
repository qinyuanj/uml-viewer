-- Isolated native acceptance driver; exercises the actual cimgui draw list.
local B=require('uml.adapters.class_boxes')
local G=require('uml.domain.geom')
local E=require('uml.edn')
local function probe(s)
  local imgui=require('cimgui'); local list=imgui.GetBackgroundDrawList_Nil()
  local c=G.get(s.scene,'classes')[1]
  local isolated=G.map{size=G.get(s.scene,'size'),packages=E.vector(),classes=E.vector{c}}
  -- Compare actual upright/slanted vertices from the same native glyph atlas.
  G.put(c,'stereotype',E.keyword('class'))
  local start=list.VtxBuffer.Size; B.new(imgui).draw(isolated,1100,760,{zoom=1,cam_x=0,cam_y=0})
  local finish=list.VtxBuffer.Size; local plain={}
  for i=start,finish-1 do plain[#plain+1]={x=list.VtxBuffer.Data[i].pos.x,y=list.VtxBuffer.Data[i].pos.y} end
  G.put(c,'stereotype',E.keyword('abstract'))
  -- Alpha adds lines before name, so compare a name-only class scene.
  local lines=G.get(c,'lines'); G.put(c,'lines',E.vector{G.map{kind=E.keyword('name'),text='Abstract'}})
  G.put(c,'stereotype',E.keyword('class')); start=list.VtxBuffer.Size; B.new(imgui).draw(isolated,1100,760,{zoom=1}); finish=list.VtxBuffer.Size
  plain={}; for i=start,finish-1 do plain[#plain+1]={x=list.VtxBuffer.Data[i].pos.x,y=list.VtxBuffer.Data[i].pos.y} end
  G.put(c,'drill?',true); local tilted=list.VtxBuffer.Size; B.new(imgui).draw(isolated,1100,760,{zoom=1})
  assert(list.VtxBuffer.Size-tilted==#plain,'same native glyph topology')
  local changed=false; for i=1,#plain do local v=list.VtxBuffer.Data[tilted+i-1].pos; if math.abs(v.x-plain[i].x)>.1 then changed=true end; assert(v.y==plain[i].y,'style preserves glyph baseline') end
  assert(changed,'actual native glyphs must slant')
  G.put(c,'stereotype',E.keyword('abstract')); G.put(c,'lines',lines); G.put(c,'drill?',false)
  local f=assert(io.open(os.getenv('UML_VIEWER_EVIDENCE')..'/class-boxes-pass.txt','wb')); f:write('PASS native cimgui glyph vertex slant, unchanged baselines; alpha uses font-independent strokes\n'); f:close()
end
return function(frame,s)
  if frame~=2 then return end
  local draw=love.draw
  love.draw=function()
    draw()
    local imgui=require('cimgui'); imgui.NewFrame(); probe(s); imgui.Render()
    love.draw=draw
  end
end
