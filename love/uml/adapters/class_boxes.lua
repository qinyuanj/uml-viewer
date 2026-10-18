local E=require('uml.edn')
local G=require('uml.domain.geom')
local M={}
local C=require('uml.domain.config')
local function bands(c)
  return G.name(C.crap_band(C.crap_risk(G.get(c,'crap')))),G.name(C.mutation_band(C.mutation_ratio(c)))
end
local function quality(commands,c,owner)
  local r=G.get(c,'rect'); local cb,mb=bands(c)
  for i,b in ipairs{cb,mb} do commands[#commands+1]={kind='quality',owner=owner,id=G.get(c,'id'),rect=r,text=i==1 and 'C' or 'M',band=b,radius=6,x=G.right(r)-24+(i-1)*14,y=G.get(r,'y')+10} end
end
-- Adapter contract: a renderer consumes commands; no UI binding leaks into scenes.
function M.commands(scene)
  local commands={}
  for _,p in ipairs(G.get(scene,'packages')) do
    local cb,mb=bands(p); local band=(cb=='red' or mb=='red') and 'red' or (cb=='yellow' or mb=='yellow') and 'yellow' or 'green'
    commands[#commands+1]={kind='package',rect=G.get(p,'rect'),text=G.get(p,'title') or G.get(p,'label'),band=band,red=band=='red'}
    quality(commands,p,'package')
  end
  for _,c in ipairs(G.get(scene,'classes')) do
    local C=require('uml.domain.config')
    local r=G.get(c,'rect'); local cb=G.name(C.crap_band(C.crap_risk(G.get(c,'crap')))); local mb=G.name(C.mutation_band(C.mutation_ratio(c)))
    local band=(cb=='red' or mb=='red') and 'red' or (cb=='yellow' or mb=='yellow') and 'yellow' or 'green'
    commands[#commands+1]={kind='class',rect=r,shape=G.name(G.get(c,'shape')),red=band=='red',band=band,id=G.get(c,'id')}
    quality(commands,c,'class')
    local stereo=G.name(G.get(c,'stereotype'))
    if stereo=='abstract' or stereo=='interface' then commands[#commands+1]={kind='marker',rect=r,text=stereo=='abstract' and 'α' or 'I',x=G.right(r)-44,y=G.get(r,'y')+3,size=12,color=0xffffffff} end
    if G.get(c,'level')~=nil then commands[#commands+1]={kind='level',rect=r,text=tostring(G.get(c,'level')),x=G.get(r,'x')+4,y=G.get(r,'y')+3,size=10,color=0xffeeeeee} end
    local y=G.get(r,'y')+16
    for _,line in ipairs(G.get(c,'lines') or {}) do
      local kind=G.name(G.get(line,'kind'))
      if kind=='rule' then commands[#commands+1]={kind='rule',x=G.get(r,'x'),y=y-4,w=G.get(r,'w')}; y=y+12
      else
        local italic=kind=='name' and ((stereo~=nil and stereo~='class') or G.get(c,'drill?')==true or #(G.get(c,'contents') or {})>0)
        commands[#commands+1]={kind='text',role=kind,italic=italic,size=kind=='name' and 14 or 12,x=G.get(r,'x')+12,y=y,text=G.get(line,'text') or ''}; y=y+18
      end
    end
  end
  return commands
end
function M.new(imgui)
  local function vec(x,y) return imgui.ImVec2_Float(x,y) end
  return {draw=function(scene,width,height)
    local size=G.get(scene,'size'); local scale=math.min(1,width/G.get(size,'w'),height/G.get(size,'h'))
    local list=imgui.GetBackgroundDrawList_Nil()
    local function point(x,y) return vec(x*scale,y*scale) end
    for _,c in ipairs(M.commands(scene)) do
      if c.kind=='package' or c.kind=='class' then
        local r=c.rect; local x,y,w,h=G.get(r,'x'),G.get(r,'y'),G.get(r,'w'),G.get(r,'h')
        local color=c.red and 0xff5555dd or c.band=='yellow' and 0xff55ccee or 0xff77bb77
        if c.shape=='oval' then list:AddEllipse(point(x+w/2,y+h/2),w/2*scale,h/2*scale,color,0,0,1.5)
        else list:AddRect(point(x,y),point(x+w,y+h),color,0,0,1.5) end
        if c.kind=='package' then list:AddText_FontPtr(nil,13*scale,point(x+12,y+8),0xffdddddd,c.text or '',nil,0,nil) end
      elseif c.kind=='quality' then
        local color=c.band=='red' and 0xff5555dd or c.band=='yellow' and 0xff55ccee or 0xff77bb77
        list:AddCircleFilled(point(c.x,c.y),c.radius*scale,color,0)
        list:AddText_FontPtr(nil,9*scale,point(c.x-3,c.y-5),0xffffffff,c.text,nil,0,nil)
      elseif c.kind=='rule' then list:AddLine(point(c.x,c.y),point(c.x+c.w,c.y),0xff777777,1)
      elseif c.kind=='marker' and c.text=='α' then
        -- Stroke the small alpha explicitly; ImGui's default atlas is Latin-only.
        local pts={{8,3},{6,2},{3,2},{1,4},{1,7},{3,9},{6,9},{8,6},{9,2},{8,7},{9,9},{11,9}}
        for i=2,#pts do local a,b=pts[i-1],pts[i]; list:AddLine(point(c.x+a[1],c.y+a[2]),point(c.x+b[1],c.y+b[2]),c.color,scale) end
      elseif c.kind=='text' or c.kind=='marker' or c.kind=='level' then
        local size=(c.size or 13)*scale; local origin=point(c.x,c.y)
        local first=c.italic and list.VtxBuffer.Size
        list:AddText_FontPtr(nil,size,origin,c.color or 0xffeeeeee,c.text,nil,0,nil)
        -- Shear only this text's newly emitted glyphs: portable oblique styling
        -- without loading or redistributing a platform-specific italic font.
        if first then for i=first,list.VtxBuffer.Size-1 do local pos=list.VtxBuffer.Data[i].pos; pos.x=pos.x+(origin.y+size-pos.y)*.2 end end
      end
    end
  end}
end
return M
