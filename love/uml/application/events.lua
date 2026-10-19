-- Pure application transitions. State is a Lua record; documents, scenes, focus
-- and extra options retain their tagged EDN representation from document.new.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local D=require('uml.application.document')
local H=require('uml.domain.hierarchy')
local P=require('uml.domain.policy')
local M={}
local K,V=E.keyword,E.vector
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
local function with_proposals(doc,proposals)
  local out=G.copy(doc); E.assoc(out,K('proposals'),proposals); return out
end
local function persist(s,doc,opts)
  opts=opts or {}
  if opts.boundary then doc=D.write_proposals(s.path,doc,opts.boundary) end
  local n=copy(s); n.document=doc; return rebuild(n)
end
-- User creation is the only path that default-names a proposal. The name is a
-- local timestamp; `opts.now` keeps headless callers deterministic.
function M.add_proposal(s,opts)
  opts=opts or {}; if not s.document then return s end
  local now=opts.now or os.time(); local base='p-'..tostring(now); local id=K(base); local n=0
  while P.proposal_by_id(s.document,id) do n=n+1; id=K(base..'-'..n) end
  local proposals=V(); for _,p in ipairs(H.named_proposals(s.document)) do proposals[#proposals+1]=p end
  proposals[#proposals+1]=G.map{id=id,name=P.timestamp_name(now),layers=V(),notice=P.proposal_notice}
  local state=copy(s); state.extra=G.copy(s.extra); state.focus=V()
  E.assoc(state.extra,K('proposal-id'),id); E.assoc(state.extra,K('proposal'),true)
  return persist(state,with_proposals(s.document,proposals),opts)
end
function M.rename_proposal(s,id,new_name,opts)
  if id==nil or new_name==nil or s.document==nil then return s end
  local name=tostring(new_name); if name:match('^%s*$') then return s end
  local proposals=V()
  for _,p in ipairs(H.named_proposals(s.document)) do
    local out=G.copy(p); if E.equal(G.get(p,'id'),id) then E.assoc(out,K('name'),name) end
    proposals[#proposals+1]=out
  end
  return persist(s,with_proposals(s.document,proposals),opts)
end
function M.delete_proposal(s,id,opts)
  if id==nil or s.document==nil then return s end
  local proposals=V()
  for _,p in ipairs(H.named_proposals(s.document)) do if not E.equal(G.get(p,'id'),id) then proposals[#proposals+1]=p end end
  local state=copy(s)
  if E.equal(G.get(s.extra,'proposal-id'),id) then
    state.extra=G.copy(s.extra); E.assoc(state.extra,K('proposal-id'),E.nil_value); E.assoc(state.extra,K('proposal'),false)
  end
  return persist(state,with_proposals(s.document,proposals),opts)
end
function M.rename_op(id,raw)
  if raw==nil then return nil end
  local name=tostring(raw):match('^%s*(.-)%s*$'); if name=='' then return nil end
  return G.map{op=K('rename'),id=id,name=name}
end
function M.apply_proposal_op(s,op,opts)
  local kind=G.name(G.get(op,'op'))
  if kind=='rename' then return M.rename_proposal(s,G.get(op,'id'),G.get(op,'name'),opts) end
  if kind=='delete' then return M.delete_proposal(s,G.get(op,'id'),opts) end
  return s
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
  if key=='escape' or key=='esc' then return M.escape(s) end
  if key=='p' or key=='P' then return M.show_proposal(s,nil) end
  if key=='r' then local n=copy(s); n.waiting=nil; n.mtime=0; n.metrics_stamp=nil; return n end
  return s
end
function M.layer_id(hit)
  local kind=G.name(G.get(hit,'kind'))
  if kind=='class' and G.get(hit,'drill?') then return G.get(hit,'id') end
  if kind=='child' then return G.get(hit,'parent') end
  if kind=='package' and H.proposal_package_id(G.get(hit,'id')) then return G.get(hit,'id') end
end
-- The separate detail window opens for the class behind a selection: a class
-- itself, or the parent of a child row or port. Other hits open nothing.
function M.detail_id_for(hit)
  local kind=G.name(G.get(hit,'kind'))
  if kind=='class' then return G.get(hit,'id') end
  if kind=='child' or kind=='port' then return G.get(hit,'parent') end
end
function M.select(s,hit)
  local n=copy(s); n.selected=hit
  local id=M.detail_id_for(hit)
  -- Switching the detail target restarts its scroll at the top.
  if id and (not s.detail_id or not E.equal(id,s.detail_id)) then n.detail_scroll=0 end
  return n
end
function M.open_detail(s,hit)
  local n=M.select(s,hit); local id=M.detail_id_for(hit); if id then n.detail_id=id end; return n
end
-- Right-click opens the element mail menu for a class, a child row, or a
-- collapsed proposal package; other selections keep the canvas menu closed.
function M.element_menu(selection)
  local kind=G.name(G.get(selection,'kind'))
  return kind=='class' or kind=='child' or M.layer_id(selection)~=nil
end
function M.on_move(s,x,y)
  local n=copy(s); local wx,wy=M.world_xy(s,x,y)
  n.hover=require('uml.engine.hit').at(s.scene,wx,wy); n.pointer={x,y}; return n
end
function M.on_press(s,x,y)
  if (G.get(s.extra,'open-layer') or #(s.focus or {})>0) and y<44 and x<320 then return M.back(s) end
  local wx,wy=M.world_xy(s,x,y); local hit=require('uml.engine.hit').at(s.scene,wx,wy)
  local kind=G.name(G.get(hit,'kind'))
  if kind=='child' or kind=='port' then return M.open_detail(s,hit) end
  return M.select(s,hit)
end
function M.on_double_press(s,x,y)
  if (G.get(s.extra,'open-layer') or #(s.focus or {})>0) and y<44 and x<320 then return M.back(s) end
  local wx,wy=M.world_xy(s,x,y); local hit=require('uml.engine.hit').at(s.scene,wx,wy)
  local layer=M.layer_id(hit); if layer then return M.drill(s,layer) end
  return M.open_detail(s,hit)
end
-- Escape keeps its existing meaning: back one level, else clear the selection.
-- The detail window closes through its own close control (close_detail).
function M.escape(s)
  if G.get(s.extra,'open-layer') or #(s.focus or {})>0 then return M.back(s) end
  local n=copy(s); n.selected=nil; return n
end
function M.close_detail(s) local n=copy(s); n.detail_id=nil; return n end
return M
