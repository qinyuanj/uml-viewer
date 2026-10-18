-- Runs inside the isolated actual LOVE viewer through its existing probe boundary.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local Atomic=require('uml.adapters.atomic_file')
local root=os.getenv('UML_MAIL_TEST_ROOT')
local evidence=os.getenv('UML_VIEWER_EVIDENCE')
local function save(name,value) Atomic.write(evidence..'/'..name,E.encode(value)) end
local function queue(text) Atomic.write(root..'/.uml-viewer/to-viewer.edn',text) end
return function(frame,state,hooks)
  if os.getenv('UML_MAIL_TEST_TRACE') then io.stderr:write('MAILTRACE frame '..frame..'\n'); io.stderr:flush() end
  if frame==4 then
    assert(hooks and hooks.mail,'actual viewer mail hook missing')
    queue('{:next-id 3 :queue [{:id 1 :op :context :context :proposal :proposal-id :p :name "中文"} {:id 2 :op :display :path "second.edn"}]}')
  elseif frame==7 then
    assert(state.mail_seen==2 and not state.waiting)
    assert(G.get(state.document,'title')=='Second diagram')
    assert(G.name(G.get(state.discussion_context,'context'))=='proposal')
    save('native-displayed.edn',G.map{path=state.path,context=state.discussion_context,seen=state.mail_seen})
    local selected=E.parse('{:kind :class :id :target}')
    for _,entry in ipairs(require('uml.adapters.mail_menu').entries()) do hooks.mail.menu_action(state,selected,entry.op) end
    local f=assert(io.open(root..'/.uml-viewer/to-agent.edn','rb')); local outbound=E.parse(f:read('*a')); f:close()
    -- second.edn has no proposal :p, so browsing drops it; :context remains :p.
    assert(E.equal(outbound,E.parse('{:next-id 5 :queue [{:id 1 :op :refresh-crap :target {:id :target :kind :class :ns "app.target"}} {:id 2 :op :refresh-mutate :target {:id :target :kind :class :ns "app.target"}} {:id 3 :op :refresh-mutate-all :target {:id :target :kind :class :ns "app.target"}} {:id 4 :op :omit :target {:id :target :kind :class :ns "app.target"}}]}')))
    save('native-menu-contract.edn',outbound)
    queue('{:next-id 5 :queue [{:id 3 :op :context :context :real} {:id 4 :op :quit-for-restart}]}')
  end
end
