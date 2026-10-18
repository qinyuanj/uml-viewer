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
    local view
    if extra==true or g(extra,'proposal') then
      view=g(extra,'open-layer') and H.layer_view(doc,g(extra,'proposal-id'),g(extra,'open-layer')) or H.proposal_view(doc,g(extra,'proposal-id'))
    else view=H.view_at(doc,focus) end
    return Compose.compile_diagram(H.apply_declutter(view,g(extra,'declutter')))
  end
  return Compose.compile_document(doc)
end
-- A rejected reload leaves the last usable scene and timestamp intact for retry.
function M.new(doc,mtime,focus,extra,snapshots,metrics_stamp)
  return {document=doc,mtime=mtime,metrics_stamp=metrics_stamp,focus=focus or V(),extra=extra or E.map(),scene=M.compile_view(doc,snapshots,focus,extra),revision=1}
end
function M.reload(state,text,mtime,snapshots,metrics_stamp)
  if mtime==state.mtime and metrics_stamp==state.metrics_stamp then return state,false end
  local ok,next_state=pcall(function() local doc=require('uml.domain.ir').read_document(text); local s=M.new(doc,mtime,state.focus,state.extra,snapshots,metrics_stamp); s.revision=state.revision+1; return s end)
  if ok then return next_state,true end
  return state,false,tostring(next_state)
end
return M
