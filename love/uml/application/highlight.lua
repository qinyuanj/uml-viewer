-- Relationship highlighting rules shared by the class box and edge renderers
-- (draw.clj selected-box-id / hovered-box-id / edge-hot?). Pure: hit maps and
-- scene edges in, ids and predicates out.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local M={}
-- The class a hit stands for: the class itself, or the parent of a child row
-- or port. Packages, edges and dependency triangles focus no class.
function M.box_id(hit)
  local kind=G.name(G.get(hit,'kind'))
  if kind=='class' then return G.get(hit,'id') end
  if kind=='child' or kind=='port' then return G.get(hit,'parent') end
end
function M.touches(e,id)
  if id==nil then return false end
  if E.equal(id,G.get(e,'from')) or E.equal(id,G.get(e,'to')) then return true end
  for _,via in ipairs(G.get(e,'via-ids') or {}) do if E.equal(id,via) then return true end end
  return false
end
local function same_edge(e,hit)
  return G.name(G.get(hit,'kind'))=='edge' and E.equal(G.get(e,'from'),G.get(hit,'from')) and E.equal(G.get(e,'to'),G.get(hit,'to'))
end
-- view = {selected=hit, hover=hit}. Returns the focus (selection) state:
-- focus id, the set of related class ids (encoded keys) and edge classifiers.
function M.new(scene,view)
  view=view or {}
  local focus=M.box_id(view.selected); local hover=M.box_id(view.hover)
  local related={}
  if focus then for _,e in ipairs(G.get(scene,'edges') or {}) do if M.touches(e,focus) then
    related[E.encode(G.get(e,'from'))]=true; related[E.encode(G.get(e,'to'))]=true
  end end end
  local h={focus=focus,hover=hover}
  function h.related(id) return focus~=nil and not E.equal(id,focus) and related[E.encode(id)]==true end
  function h.dimmed(id) return focus~=nil and not E.equal(id,focus) and not related[E.encode(id)] end
  function h.hot(e) return M.touches(e,focus) or same_edge(e,view.hover) end
  function h.warm(e) return M.touches(e,hover) end
  return h
end
return M
