local E=require('uml.edn')
local G=require('uml.domain.geom')
local M={}
-- Adapter contract: a renderer consumes commands; no UI binding leaks into scenes.
function M.commands(scene)
  local commands={}
  for _,p in ipairs(G.get(scene,'packages')) do commands[#commands+1]={kind='package',rect=G.get(p,'rect'),text=G.get(p,'label')} end
  for _,c in ipairs(G.get(scene,'classes')) do
    local C=require('uml.domain.config')
    local r=G.get(c,'rect'); local cb=G.name(C.crap_band(C.crap_risk(G.get(c,'crap')))); local mb=G.name(C.mutation_band(C.mutation_ratio(c)))
    local band=(cb=='red' or mb=='red') and 'red' or (cb=='yellow' or mb=='yellow') and 'yellow' or 'green'
    commands[#commands+1]={kind='class',rect=r,shape=G.name(G.get(c,'shape')),red=band=='red',band=band,id=G.get(c,'id')}
    local y=G.get(r,'y')+16
    for _,line in ipairs(G.get(c,'lines') or {}) do
      local kind=G.name(G.get(line,'kind'))
      if kind=='rule' then commands[#commands+1]={kind='rule',x=G.get(r,'x'),y=y-4,w=G.get(r,'w')}; y=y+12
      else commands[#commands+1]={kind='text',x=G.get(r,'x')+12,y=y,text=G.get(line,'text') or ''}; y=y+18 end
    end
  end
  return commands
end
function M.new(imgui)
  local function vec(x,y) return imgui.ImVec2_Float(x,y) end
  return {draw=function(scene,width,height,view)
    local size=G.get(scene,'size'); local scale=view and (view.zoom or 1) or math.min(1,width/G.get(size,'w'),height/G.get(size,'h'))
    local list=imgui.GetBackgroundDrawList_Nil()
    local function point(x,y) return vec((x-(view and view.cam_x or 0))*scale,(y-(view and view.cam_y or 0))*scale) end
    for _,c in ipairs(M.commands(scene)) do
      if c.kind=='package' or c.kind=='class' then
        local r=c.rect; local x,y,w,h=G.get(r,'x'),G.get(r,'y'),G.get(r,'w'),G.get(r,'h')
        local color=c.kind=='package' and 0xff514b45 or (c.red and 0xff5555dd or c.band=='yellow' and 0xff55ccee or 0xff77bb77)
        if c.shape=='oval' then list:AddEllipse(point(x+w/2,y+h/2),w/2*scale,h/2*scale,color,0,0,1.5)
        else list:AddRect(point(x,y),point(x+w,y+h),color,0,0,1.5) end
        if c.kind=='package' then list:AddText_FontPtr(nil,13*scale,point(x+12,y+8),0xffdddddd,c.text or '',nil,0,nil) end
      elseif c.kind=='rule' then list:AddLine(point(c.x,c.y),point(c.x+c.w,c.y),0xff777777,1)
      elseif c.kind=='text' then list:AddText_FontPtr(nil,13*scale,point(c.x,c.y),0xffeeeeee,c.text,nil,0,nil) end
    end
  end}
end
return M
