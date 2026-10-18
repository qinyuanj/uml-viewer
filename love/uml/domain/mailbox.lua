-- Pure EDN mailbox protocol. Stored queue order is authoritative, never id sorting.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local M={}
local g=G.get
function M.normalize(raw)
  if E.kind(g(raw,'queue'))=='vector' then return G.map{['next-id']=g(raw,'next-id') or 1,queue=g(raw,'queue')} end
  if g(raw,'op') then return G.map{['next-id']=(g(raw,'id') or 0)+1,queue=E.vector{raw}} end
  return G.map{['next-id']=1,queue=E.vector()}
end
function M.pending(box,seen)
  local q=E.vector(); for _,cmd in ipairs(g(box,'queue')) do if g(cmd,'id')>(seen or 0) then q[#q+1]=cmd end end; return q
end
function M.take(box,seen)
  local q=M.pending(box,seen); if #q==0 then return nil end
  local rest=E.vector(); for i=2,#q do rest[#rest+1]=q[i] end
  return q[1],G.map{['next-id']=g(box,'next-id'),queue=rest}
end
function M.append(box,op,extra)
  local id=g(box,'next-id'); local cmd=G.map{id=id,op=type(op)=='string' and E.keyword(op) or op}
  for _,entry in ipairs(extra or {}) do E.assoc(cmd,entry[1],entry[2]) end
  local q=E.vector(); for _,c in ipairs(g(box,'queue')) do q[#q+1]=c end; q[#q+1]=cmd
  return cmd,G.map{['next-id']=id+1,queue=q}
end
function M.last_id(box) local q=g(box,'queue'); return q[#q] and g(q[#q],'id') or 0 end
return M
