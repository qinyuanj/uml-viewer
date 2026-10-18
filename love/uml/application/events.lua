-- Pure application transitions. State is a Lua record; documents, scenes, focus
-- and extra options retain their tagged EDN representation from document.new.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local D=require('uml.application.document')
local H=require('uml.domain.hierarchy')
local M={}
local function copy(s) local n={}; for k,v in pairs(s) do n[k]=v end; return n end
local function rebuild(s)
  if s.document and G.get(s.document,'hierarchical') then
    s.scene=D.compile_view(s.document,s.snapshots,s.focus,s.extra)
    s.cam_x=0; s.cam_y=0; s.selected=nil
  end
  return s
end
function M.show_proposal(s,id)
  if id then local found=false; for _,p in ipairs(H.named_proposals(s.document)) do if E.equal(id,G.get(p,'id')) then found=true end end; if not found then return s end end
  local n=copy(s); n.focus=E.vector(); n.extra=G.copy(s.extra)
  E.assoc(n.extra,E.keyword('proposal-id'),id or E.nil_value); E.assoc(n.extra,E.keyword('proposal'),id~=nil); E.assoc(n.extra,E.keyword('open-layer'),E.nil_value)
  return rebuild(n)
end
function M.cycle_declutter(s)
  local n=copy(s); n.extra=G.copy(s.extra); E.assoc(n.extra,E.keyword('declutter'),H.next_declutter(G.get(s.extra,'declutter'))); return rebuild(n)
end
function M.drill(s,id)
  local n=copy(s); n.focus=E.vector()
  if H.proposal_package_id(id) or H.find_nested_group(s.document,G.get(s.extra,'proposal-id'),id) then
    n.extra=G.copy(s.extra); E.assoc(n.extra,E.keyword('open-layer'),id)
  else
    for _,x in ipairs(s.focus or {}) do n.focus[#n.focus+1]=x end
    n.focus[#n.focus+1]=E.keyword(G.name(id):match('[^.]+$'))
  end
  return rebuild(n)
end
function M.back(s)
  local layer=G.get(s.extra,'open-layer'); if not layer and #(s.focus or {})==0 then return s end
  local n=copy(s)
  if layer then n.extra=G.copy(s.extra); E.assoc(n.extra,E.keyword('open-layer'),E.nil_value)
  else n.focus=E.vector(); for i=1,#s.focus-1 do n.focus[i]=s.focus[i] end end
  return rebuild(n)
end
function M.zoom_of(s) return s.zoom or 1 end
function M.on_scroll(s,amount,dims)
  dims=type(dims)=='table' and dims or {window_h=dims}; local n=copy(s)
  amount=type(amount)=='number' and amount or type(amount)=='table' and amount.count or 0
  local z=M.zoom_of(s); local size=G.get(s.scene,'size')
  local horizontal=dims.horizontal; local axis=horizontal and 'cam_x' or 'cam_y'
  local minimum=G.get(size,horizontal and 'min-x' or 'min-y') or 0
  local visible=horizontal and (dims.view_w or dims.window_w or 1500) or (dims.window_h or 900)
  local maximum=math.max(minimum,(G.get(size,horizontal and 'w' or 'h') or (horizontal and 800 or 600))-visible/z)
  n[axis]=math.max(minimum,math.min(maximum,(s[axis] or 0)+amount*48/z)); return n
end
function M.world_xy(s,x,y) local z=M.zoom_of(s); return (s.cam_x or 0)+x/z,(s.cam_y or 0)+y/z end
local function key_name(x) if type(x)=='string' then return x end; if E.kind(x)=='keyword' then return G.name(x) end end
function M.zoom_dir(key,dims)
  dims=dims or {}; key=key_name(key); local c=dims.key_code; local raw=type(dims.raw_key)=='string' and dims.raw_key:sub(1,1)
  if key=='+' or key=='=' or key=='kp+' or c==61 or c==107 or c==521 or raw=='+' or raw=='=' then return 'in' end
  if key=='-' or key=='_' or key=='minus' or key=='subtract' or key=='kp-' or c==45 or c==109 or raw=='-' or raw=='_' or raw==string.char(31) then return 'out' end
  if key=='0' or key=='kp0' or c==48 or c==96 or raw=='0' then return 'reset' end
end
function M.on_key(s,key,dims)
  dims=dims or {}; key=key_name(key)
  local dir=dims.control and M.zoom_dir(key,dims)
  if dir then
    local n=copy(s); local old=M.zoom_of(s); local z=dir=='reset' and 1 or dir=='in' and old*1.1 or old/1.1
    n.zoom=math.max(.1,math.min(10,z)); local w=dims.view_w or dims.window_w or 1500; local h=dims.window_h or 900
    n.cam_x=(s.cam_x or 0)+w/(2*old)-w/(2*n.zoom); n.cam_y=(s.cam_y or 0)+h/(2*old)-h/(2*n.zoom); return n
  end
  if key=='left' or key=='right' then local d=copy(dims); d.horizontal=true; return M.on_scroll(s,key=='left' and -2 or 2,d) end
  if key=='up' or key=='down' then local d=copy(dims); d.horizontal=false; return M.on_scroll(s,key=='up' and -2 or 2,d) end
  if key=='escape' or key=='esc' then
    if G.get(s.extra,'open-layer') or #(s.focus or {})>0 then return M.back(s) end
    local n=copy(s); n.selected=nil; return n
  end
  if key=='r' then local n=copy(s); n.waiting=nil; n.mtime=0; n.metrics_stamp=nil; return n end
  return s
end
function M.layer_id(hit)
  local kind=G.name(G.get(hit,'kind'))
  if kind=='class' and G.get(hit,'drill?') then return G.get(hit,'id') end
  if kind=='child' then return G.get(hit,'parent') end
  if kind=='package' and H.proposal_package_id(G.get(hit,'id')) then return G.get(hit,'id') end
end
function M.on_move(s,x,y)
  local n=copy(s); local wx,wy=M.world_xy(s,x,y)
  n.hover=require('uml.engine.hit').at(s.scene,wx,wy); n.pointer={x,y}; return n
end
function M.on_press(s,x,y)
  if (G.get(s.extra,'open-layer') or #(s.focus or {})>0) and y<44 and x<320 then return M.back(s) end
  local n=copy(s); local wx,wy=M.world_xy(s,x,y); n.selected=require('uml.engine.hit').at(s.scene,wx,wy); return n
end
return M
