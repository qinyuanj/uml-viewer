-- Exact legacy element menu contract; ImGui calls stay at this adapter boundary.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local Events=require('uml.application.events')
local M={}
function M.entries()
  return {{label='Refresh CRAP',op='refresh-crap'},{label='Refresh Mutation',op='refresh-mutate'},
    {label='Refresh All Mutation',op='refresh-mutate-all'},{label='Omit',op='omit'}}
end
local function classes(doc,into)
  into=into or {}
  for _,c in ipairs(G.get(doc,'classes') or {}) do
    into[#into+1]=c
    if G.get(c,'classes') then classes(c,into) end
  end
  for _,p in ipairs(G.get(doc,'packages') or {}) do into[#into+1]=p; classes(p,into) end
  for _,d in ipairs(G.get(doc,'diagrams') or {}) do classes(d,into) end
  return into
end
function M.target(state,selection)
  local id=G.get(selection,'id'); local class
  for _,c in ipairs(classes(state.document or state.scene)) do if E.equal(id,G.get(c,'id')) then class=c; break end end
  local component=Events.layer_id(selection) or G.get(class,'dummy?') or #(G.get(class,'contents') or {})>0 or G.name(G.get(selection,'kind'))=='package'
  local target=G.map{kind=E.keyword(component and 'component' or 'class'),id=id or E.nil_value}
  G.put(target,'ns',G.get(class,'ns')); G.put(target,'proposal-id',G.get(state.extra,'proposal-id')); return target
end
function M.draw(imgui,state,selection,send)
  if imgui.BeginPopup('uml-element-mail') then
    for _,entry in ipairs(M.entries()) do if imgui.MenuItem_Bool(entry.label,nil,false,true) then send(entry.op,G.map{target=M.target(state,selection)}) end end
    imgui.EndPopup()
  end
end
return M
