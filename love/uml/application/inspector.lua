-- Inspector commands describe the declared document; choosing changes browsing
-- and returns the explicit discussion context for the mail adapter to publish.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local H=require('uml.domain.hierarchy')
local Events=require('uml.application.events')
local Detail=require('uml.application.detail')
local S=require('uml.application.strings')
local M={}
-- The fixed right sidebar: the canvas extent and horizontal hit tests subtract
-- this width (adapters pass it through dims.view_w; hit.clamp ignores presses
-- inside the sidebar). The legacy geometry constants below only describe rows
-- inside the sidebar itself.
M.width=280
M.sidebar_w=280
function M.canvas_w(window_w) return math.max(0,(window_w or 0)-M.width) end
function M.canvas_dims(window_w,window_h) return {window_w=window_w,window_h=window_h,view_w=M.canvas_w(window_w)} end
function M.in_canvas(x,window_w) return x>=0 and x<M.canvas_w(window_w) end
M.inspector_pad=16
M.inspector_row_h=22
M.inspector_btn_h=28
M.inspector_btn_gap=8
M.real_diagram_y=40
M.proposals_label_y=66
M.proposal_rows_y=82
M.empty_message=S.help
-- The explanation occupies the detail area only while nothing is selected.
function M.empty_explanation(state)
  if state==nil or state.selected==nil then return M.empty_message end
end
function M.inspector_x(window_w) return window_w-M.sidebar_w+M.inspector_pad end
function M.inspector_inner_w() return M.sidebar_w-2*M.inspector_pad end
function M.real_diagram_rect(window_w) return G.rect(M.inspector_x(window_w),M.real_diagram_y,M.inspector_inner_w(),M.inspector_row_h) end
function M.proposal_row_rect(window_w,i) return G.rect(M.inspector_x(window_w),M.proposal_rows_y+i*M.inspector_row_h,M.inspector_inner_w(),M.inspector_row_h) end
function M.new_proposal_rect(window_w,n) return G.rect(M.inspector_x(window_w),M.proposal_rows_y+n*M.inspector_row_h+M.inspector_btn_gap,M.inspector_inner_w(),M.inspector_btn_h) end
function M.declutter_rect(window_w,n)
  local r=M.new_proposal_rect(window_w,n); return G.rect(G.get(r,'x'),G.get(r,'y')+M.inspector_btn_h+M.inspector_btn_gap,G.get(r,'w'),G.get(r,'h'))
end
function M.regen_button(window_w,window_h) return G.rect(window_w-M.sidebar_w+16,window_h-56,M.sidebar_w-32,32) end
function M.in_rect(r,x,y) return r~=nil and x>=G.get(r,'x') and x<G.right(r) and y>=G.get(r,'y') and y<G.bottom(r) end
function M.regen_hit(x,y,window_w,window_h) return M.in_rect(M.regen_button(window_w,window_h),x,y) end
function M.commands(state)
  local active=G.get(state.extra,'proposal-id')
  local out={{kind='real',label=G.get(state.document,'title') or S.real_diagram,active=not active}}
  for _,p in ipairs(H.named_proposals(state.document)) do local id=G.get(p,'id'); out[#out+1]={kind='proposal',id=id,label=G.get(p,'name') or G.name(id),active=E.equal(active,id)} end
  return out
end
-- The button names the current declutter mode, as the original does, so the
-- user can read the active state without opening anything.
function M.declutter_label(state)
  local mode=G.name(G.get(state.extra,'declutter') or E.keyword('full'))
  return S.declutter_modes[mode] or S.declutter
end
-- Screen-space hit test for the inspector list: proposal rows first, then the
-- real-diagram link, New Proposal, and Declutter. Nil means no inspector hit.
function M.hit(state,x,y,window_w)
  local proposals=H.named_proposals(state.document)
  for i=0,#proposals-1 do
    if M.in_rect(M.proposal_row_rect(window_w,i),x,y) then
      return G.map{kind=E.keyword('proposal'),id=G.get(proposals[i+1],'id'),index=i}
    end
  end
  if M.in_rect(M.real_diagram_rect(window_w),x,y) then return G.map{kind=E.keyword('real-diagram')} end
  if M.in_rect(M.new_proposal_rect(window_w,#proposals),x,y) then return G.map{kind=E.keyword('new-proposal')} end
  if M.in_rect(M.declutter_rect(window_w,#proposals),x,y) then return G.map{kind=E.keyword('declutter')} end
end
function M.press(state,hit,opts)
  local kind=G.name(G.get(hit,'kind'))
  if kind=='real-diagram' then return Events.show_proposal(state,nil) end
  if kind=='proposal' then return Events.show_proposal(state,G.get(hit,'id')) end
  if kind=='new-proposal' then return Events.add_proposal(state,opts) end
  if kind=='declutter' then return Events.cycle_declutter(state) end
  return state
end
function M.menu(state,id,pointer)
  pointer=pointer or {}
  local found
  for _,p in ipairs(H.named_proposals(state.document)) do if E.equal(G.get(p,'id'),id) then found=p end end
  if not found then return nil end
  local entries=E.vector()
  entries[1]=G.map{label=S.rename,op=E.keyword('rename')}
  entries[2]=G.map{label=S.delete,op=E.keyword('delete')}
  return G.map{id=id,name=G.get(found,'name'),anchor=G.map{x=pointer.x or 0,y=pointer.y or 0},entries=entries}
end
function M.menu_action(state,op,id,new_name,opts)
  local kind=G.name(op)
  if kind=='rename' then
    local operation=Events.rename_op(id,new_name)
    if not operation then return state end
    return Events.apply_proposal_op(state,operation,opts)
  end
  if kind=='delete' then return Events.delete_proposal(state,id,opts) end
  return state
end
-- Detail-card wheel input: the card scroll offset is application state, and
-- clamping uses the rows and the viewport height reported by the adapter.
function M.scroll(state,delta,view_h)
  local n={}; for k,v in pairs(state) do n[k]=v end
  local id=state.detail_id or G.get(state.selected,'id'); local model=id and Detail.for_state(state,id)
  local rows=model and Detail.rows(model)
  n.detail_scroll=Detail.scroll(rows,view_h,state.detail_scroll or 0,delta)
  return n
end
-- Search authored identities, never collapsed visible boxes. UTF-8 substrings
-- remain intact; ASCII namespace matching is case insensitive.
function M.search(state,query)
  local out={}; query=tostring(query or ''):match('^%s*(.-)%s*$'):lower()
  if query=='' then return out end
  local function add(c)
    local id=G.get(c,'id'); if not id then return end
    local name=G.get(c,'name') or G.name(id)
    local ns=G.get(c,'ns') or G.name(id)
    if name:lower():find(query,1,true) or ns:lower():find(query,1,true) then out[#out+1]={id=id,name=name,ns=ns,label=name..'  ·  '..ns} end
  end
  for _,c in ipairs(G.get(state.document,'classes') or {}) do add(c) end
  for _,p in ipairs(G.get(state.document,'packages') or {}) do for _,c in ipairs(G.get(p,'classes') or {}) do add(c) end end
  table.sort(out,function(a,b) if a.ns~=b.ns then return a.ns<b.ns end; return E.encode(a.id)<E.encode(b.id) end)
  return out
end
function M.choose(state,id)
  for _,c in ipairs(M.commands(state)) do if (not id and c.kind=='real') or (id and E.equal(id,c.id)) then
    local context=id and G.map{context=E.keyword('proposal'),['proposal-id']=id,name=c.label} or G.map{context=E.keyword('real')}
    return Events.show_proposal(state,id),context
  end end
  return state
end
return M
