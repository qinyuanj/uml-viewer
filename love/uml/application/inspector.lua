-- Inspector commands describe the declared document; choosing changes browsing
-- and returns the explicit discussion context for the mail adapter to publish.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local H=require('uml.domain.hierarchy')
local Events=require('uml.application.events')
local M={}
function M.commands(state)
  local active=G.get(state.extra,'proposal-id')
  local out={{kind='real',label=G.get(state.document,'title') or 'Real diagram',active=not active}}
  for _,p in ipairs(H.named_proposals(state.document)) do local id=G.get(p,'id'); out[#out+1]={kind='proposal',id=id,label=G.get(p,'name') or G.name(id),active=E.equal(active,id)} end
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
