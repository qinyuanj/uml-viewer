-- Companion mail IO adapter. Wake-up is optional; durable queue is authoritative.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local D=require('uml.application.document')
local Boxes=require('uml.adapters.mailbox')
local Menu=require('uml.adapters.mail_menu')
local M={}
function M.new(root,options)
  options=options or {}
  local inbox=options.inbox or Boxes.new(root)
  local outbox=options.outbox or Boxes.open(root..'/.uml-viewer/to-agent.edn')
  local api={}
  function api.request(op,extra)
    local cmd=outbox.append(op,extra)
    if options.notify then pcall(options.notify,cmd) end
    return cmd
  end
  function api.send_context(state,context)
    local cmd=api.request('context',context)
    local s={}; for k,v in pairs(state) do s[k]=v end; s.discussion_context=G.copy(context)
    return s,cmd
  end
  function api.menu_action(state,selection,op) return api.request(op,G.map{target=Menu.target(state,selection)}) end
  function api.poll(state) return D.poll_mail(state,inbox,options) end
  return api
end
return M
