-- Restart session snapshots. Pure: the viewer supplies the state record and the
-- file adapter owns session.edn. The schema matches the JVM writer so either
-- runtime's saved view has the same four restoration dimensions.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local D=require('uml.application.document')
local M={}
local function get(m,k) local v,found=E.get(m or E.map(),E.keyword(k)); if found then return v end end
local function vector_copy(items)
  local out=E.vector(); for i=1,#(items or {}) do out[i]=items[i] end; return out
end
-- Window chrome (#84) travels beside the view: window px size and the
-- DPI-independent inspector width. The viewer adapter owns it, so a same- or
-- different-diagram :display never resets it.
local chrome_keys={{'window_w','window-w'},{'window_h','window-h'},{'inspector_w','inspector-width'}}
function M.chrome(snapshot)
  local out={}
  if not snapshot or E.kind(snapshot)~='map' then return out end
  for _,k in ipairs(chrome_keys) do
    local v=get(snapshot,k[2]); if type(v)=='number' and v>0 then out[k[1]]=v end
  end
  return out
end
function M.snapshot(state,chrome)
  local m=E.map()
  for _,k in ipairs(chrome_keys) do
    local v=chrome and chrome[k[1]]; if type(v)=='number' then E.assoc(m,E.keyword(k[2]),v) end
  end
  if state.path then E.assoc(m,E.keyword('path'),state.path) end
  if state.discussion_context then
    E.assoc(m,E.keyword('discussion-context'),G.copy(state.discussion_context))
  end
  E.assoc(m,E.keyword('mail-seen'),state.mail_seen or 0)
  E.assoc(m,E.keyword('focus'),vector_copy(state.focus))
  for _,key in ipairs{'proposal-id','proposal','open-layer','declutter'} do
    local value=G.get(state.extra,key); if value~=nil then E.assoc(m,E.keyword(key),value) end
  end
  E.assoc(m,E.keyword('cam-x'),state.cam_x or 0)
  E.assoc(m,E.keyword('cam-y'),state.cam_y or 0)
  E.assoc(m,E.keyword('zoom'),state.zoom or 1)
  if state.selected~=nil then E.assoc(m,E.keyword('selected'),state.selected) end
  if state.detail_id~=nil then E.assoc(m,E.keyword('detail-id'),state.detail_id) end
  return m
end
function M.path(explicit,snapshot,default)
  if explicit then return explicit end
  local saved=snapshot and get(snapshot,'path')
  if saved then return saved end
  return default
end
-- Reuses the display-reload validator so invalid focus/proposal/layer/selection
-- targets fall back exactly as they do for an in-session same-diagram display.
function M.restore(state,snapshot)
  if not snapshot or E.kind(snapshot)~='map' then return state end
  local extra=E.map()
  for _,key in ipairs{'proposal-id','proposal','open-layer','declutter'} do
    local value=get(snapshot,key); if value~=nil then E.assoc(extra,E.keyword(key),value) end
  end
  local shaped={focus=get(snapshot,'focus') or E.vector(),extra=extra,
    cam_x=get(snapshot,'cam-x'),cam_y=get(snapshot,'cam-y'),zoom=get(snapshot,'zoom'),
    selected=get(snapshot,'selected'),detail_id=get(snapshot,'detail-id')}
  local context=get(snapshot,'discussion-context'); local has_context=context~=nil
  if has_context then shaped.discussion_context=context end
  local restored=D.restore_session(state,shaped)
  if not has_context then restored.discussion_context=state.discussion_context end
  restored.mail_seen=get(snapshot,'mail-seen') or state.mail_seen or 0
  return restored
end
return M
