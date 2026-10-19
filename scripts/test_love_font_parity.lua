-- Isolated visual evidence and real-font bounds at the approved DPI1/zoom1 state.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local Events=require('uml.application.events')
local Boxes=require('uml.adapters.class_boxes')
local Detail=require('uml.application.detail')
return function(frame,state,ui)
  if frame==1 then
    if os.getenv('UML_PARITY_SCENE')=='baseline' then
      local next_state=Events.drill(Events.drill(state,E.keyword('uml')),E.keyword('application'))
      for k,v in pairs(next_state) do state[k]=v end
      state.selected=G.map{kind=E.keyword('class'),id=E.keyword('uml.application.detail')}
    else
      local next_state=Events.drill(state,E.keyword('demo'))
      for k,v in pairs(next_state) do state[k]=v end
      state.selected=G.map{kind=E.keyword('class'),id=E.keyword('demo.detail')}
    end
    state.detail_id=G.get(state.selected,'id')
    state.zoom=1; state.cam_x=0; state.cam_y=0
  elseif frame==3 then
    local w,h=love.graphics.getDimensions()
    assert(w==1500 and h==920 and love.window.getDPIScale()==1,'capture requires 1500x920 DPI1')
    local count=0
    for _,c in ipairs(Boxes.commands(state.scene)) do
      if c.kind=='text' then
        local font=c.role=='name' and ui.fonts.title or ui.fonts.body
        local size=Boxes.fit(font,math.min(c.size,c.h),c.text,c.max_w,c.bold,c.italic)
        assert(size<=18 and size<=c.h+1e-6,'text exceeds vertical slot: '..c.text)
        local measured=font:CalcTextSizeA(size,1e9,0,c.text,nil,nil)
        local width=measured.x+(c.bold and size*.05 or 0)+(c.italic and size*.2 or 0)
        assert(width<=c.max_w+1e-4,'text exceeds horizontal slot: '..c.text)
        count=count+1
      end
    end
    assert(count>0,'empty capture scene')
    if frame==3 and os.getenv('UML_PARITY_SCENE')=='baseline' then
      local f=assert(io.open(os.getenv('UML_VIEWER_EVIDENCE')..'/quality-diagnosis.txt','wb'))
      for _,c in ipairs(Boxes.commands(state.scene)) do if c.kind=='class' and E.equal(c.id,G.get(state.selected,'id')) then
        f:write('Detail missing='..tostring(c.missing)..' band='..c.band..' continuousGrade='..c.grade..' stroke='..table.concat(c.stroke,',')..'\n')
      end end
      f:close()
    end
    local rows=assert(Detail.rows(assert(Detail.model(state.scene,G.get(state.selected,'id')))),'selected detail missing')
    assert(rows[1].h==20)
    for i=2,#rows do assert(rows[i].h==18 and rows[i].y>=rows[i-1].y+rows[i-1].h) end
    local f=assert(io.open(os.getenv('UML_VIEWER_EVIDENCE')..'/parity-pass.txt','wb'))
    f:write('PASS 1500x920 DPI1 zoom1 text bounds='..count..' detail rows='..#rows..'\n'); f:close()
  elseif frame==6 then
    state.detail_id=nil
  elseif frame==8 then
    love.graphics.captureScreenshot(function(data) local out=assert(io.open(os.getenv('UML_VIEWER_EVIDENCE')..'/canvas.png','wb')); out:write(data:encode('png'):getString()); out:close() end)
  elseif frame==10 then love.event.quit(0) end
end
