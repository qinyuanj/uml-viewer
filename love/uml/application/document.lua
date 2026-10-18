-- Pure viewer document compiler: tagged IR and view options in, tagged scene out.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local H=require('uml.domain.hierarchy')
local Compose=require('uml.engine.compose')
local M={}
local g=G.get
local V=E.vector
-- snapshots is an optional immutable record array from adapters.metrics.snapshot.
-- Filesystem roots are resolved by the adapter, never by this pure compiler.
function M.compile_view(doc,snapshots,focus,extra)
  if snapshots then doc=require('uml.application.overlay').apply(doc,snapshots) end
  focus=focus or V(); extra=extra or E.map()
  if g(doc,'hierarchical') then
    local ps=H.named_proposals(doc)
    local pid=g(extra,'proposal-id') or ((extra==true or g(extra,'proposal')) and ps[1] and g(ps[1],'id'))
    local layer=g(extra,'open-layer'); local mode=g(extra,'declutter')
    local view
    if pid and layer and #focus==0 then view=H.layer_view(doc,pid,layer)
    elseif pid and #focus==0 and #ps>0 then view=H.proposal_view(doc,pid)
    else view=H.view_at(doc,focus) end
    if layer and G.name(mode)=='classes' then mode=E.keyword('elements') end
    return Compose.compile_diagram(H.apply_declutter(view,mode))
  end
  return Compose.compile_document(doc)
end
-- A rejected reload leaves the last usable scene and timestamp intact for retry.
function M.new(doc,mtime,focus,extra,snapshots,metrics_stamp)
  return {document=doc,mtime=mtime,snapshots=snapshots,metrics_stamp=metrics_stamp,focus=focus or V(),extra=extra or E.map(),scene=M.compile_view(doc,snapshots,focus,extra),revision=1}
end
function M.reload(state,text,mtime,snapshots,metrics_stamp)
  if mtime==state.mtime and metrics_stamp==state.metrics_stamp then return state,false end
  local ok,next_state=pcall(function()
    local doc=require('uml.domain.ir').read_document(text)
    local s={}; for key,value in pairs(state) do s[key]=value end
    s.document=doc; s.mtime=mtime; s.metrics_stamp=metrics_stamp; s.snapshots=snapshots
    s.scene=M.compile_view(doc,snapshots,s.focus,s.extra); s.revision=state.revision+1; return s
  end)
  if ok then return next_state,true end
  return state,false,tostring(next_state)
end
local function copy(s) local n={}; for k,v in pairs(s) do n[k]=v end; return n end
local function has_id(items,id)
  for _,item in ipairs(items or {}) do if E.equal(g(item,'id'),id) then return item end end
end
local function scene_classes(scene) return g(scene,'classes') or V() end
local function card_target(s,id)
  return has_id(scene_classes(s.scene),id) or has_id(g(s.document,'classes'),id)
    or (g(s.document,'hierarchical') and (function()
      for _,p in ipairs(g(H.view_at(s.document,V()),'packages') or {}) do if has_id(g(p,'classes'),id) then return true end end
    end)())
end
local function selected_target(s,sel)
  local kind=G.name(g(sel,'kind')); local id=g(sel,'id')
  if kind=='class' then return card_target(s,id) end
  if kind=='package' then return has_id(g(s.scene,'packages'),id) end
  if kind=='edge' then
    for _,edge in ipairs(g(s.scene,'edges') or {}) do if E.equal(g(sel,'from'),g(edge,'from')) and E.equal(g(sel,'to'),g(edge,'to')) then return true end end
  elseif kind=='child' or kind=='port' then
    local parent=has_id(scene_classes(s.scene),g(sel,'parent'))
    for _,key in ipairs{'contents','in-ports','out-ports'} do if has_id(g(parent,key),id) then return true end end
  elseif kind=='dep' then
    for _,dep in ipairs(g(s.scene,'dep-indicators') or {}) do if E.equal(id,g(dep,'id')) and E.equal(g(sel,'dir'),g(dep,'dir')) then return true end end
  elseif not kind then return card_target(s,sel) end
end
function M.drop_missing_detail(s)
  if s.detail_id and not card_target(s,s.detail_id) then s.detail_id=nil end
  if s.selected and not selected_target(s,s.selected) then s.selected=nil end
  return s
end
-- Reapply only valid view targets after same-canonical-diagram display.
function M.restore_session(state,snap)
  if not snap or not state.document then return state end
  local s=copy(state); local focus=V(); for _,x in ipairs(snap.focus or {}) do focus[#focus+1]=x end
  if g(s.document,'hierarchical') then
    while #focus>0 do
      local present=false
      for _,p in ipairs(g(H.view_at(s.document,focus),'packages') or {}) do if #(g(p,'classes') or {})>0 then present=true end end
      if present then break end; table.remove(focus)
    end
  end
  local extra=G.copy(snap.extra); local pid=g(extra,'proposal-id')
  if not has_id(H.named_proposals(s.document),pid) then pid=nil end
  local layer=g(extra,'open-layer')
  if not (pid and layer and (has_id(g(H.proposal_view(s.document,pid),'packages'),layer) or H.find_nested_group(s.document,pid,layer))) then layer=nil end
  E.assoc(extra,E.keyword('proposal-id'),pid or E.nil_value); E.assoc(extra,E.keyword('proposal'),pid~=nil)
  E.assoc(extra,E.keyword('open-layer'),layer or E.nil_value)
  s.focus=focus; s.extra=extra; s.scene=M.compile_view(s.document,s.snapshots,focus,extra)
  s.cam_x=snap.cam_x or 0; s.cam_y=snap.cam_y or 0; s.zoom=snap.zoom or 1
  s.selected=snap.selected; s.detail_id=snap.detail_id; s.discussion_context=snap.discussion_context
  return M.drop_missing_detail(s)
end
-- IO is supplied by the adapter: resolve, canonical and load. No filesystem in
-- this module. Unknown operations (including inbound omit) only advance cursor:
-- omit is an outbound companion policy request, followed by generated display.
function M.apply_mail(state,cmd,boundary)
  local s=copy(state); s.mail_seen=g(cmd,'id'); local op=G.name(g(cmd,'op'))
  if op=='display' and g(cmd,'path') then
    local path=boundary.resolve(g(cmd,'path'),state.project_root)
    local loaded=boundary.load(path)
    local same=state.path and boundary.canonical(state.path)==boundary.canonical(path)
    s=same and M.restore_session(loaded,state) or loaded
    if not same then
      -- A different diagram starts at its root view; browsing state is not
      -- transferred, but declutter and a proposal that still exists are kept.
      local old=G.get(state.extra,'declutter'); if old then E.assoc(s.extra,E.keyword('declutter'),old) end
      local pid=G.get(state.extra,'proposal-id')
      if pid and has_id(H.named_proposals(loaded.document),pid) then
        E.assoc(s.extra,E.keyword('proposal-id'),pid); E.assoc(s.extra,E.keyword('proposal'),true)
      end
      s.scene=M.compile_view(s.document,s.snapshots,s.focus,s.extra)
    end
    s.path=path; s.project_root=state.project_root; s.mail_seen=g(cmd,'id')
    s.discussion_context=state.discussion_context; s.waiting=nil
    s.cam_x=s.cam_x or 0; s.cam_y=s.cam_y or 0; s.zoom=s.zoom or 1
    s.revision=(state.revision or 0)+1
  elseif op=='context' then
    s.discussion_context=E.map()
    for _,key in ipairs{'context','proposal-id','name'} do
      local value,found=E.get(cmd,E.keyword(key)); if found then E.assoc(s.discussion_context,E.keyword(key),value) end
    end
  elseif op=='quit-for-restart' then s.quit_for_restart=true end
  return s
end
function M.poll_mail(state,mailbox,boundary)
  if not state.path then return state end
  while true do
    local cmd=mailbox.take(state.mail_seen or 0); if not cmd then return state end
    local ok,next_state=pcall(M.apply_mail,state,cmd,boundary)
    if ok then state=next_state
    else state=copy(state); state.mail_seen=g(cmd,'id'); state.error=tostring(next_state) end
  end
end
return M
