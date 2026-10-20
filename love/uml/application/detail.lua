-- Public detail card model and content-space rows; no rendering dependencies.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local S=require('uml.application.strings')
local M={width=640,height=700,pad=16,col_gap=10}
local function g(t,k) if E.kind(t)=='map' then return G.get(t,k) end; return t and t[k] end
local function named(scene,key,id) for _,c in ipairs(g(scene,key) or {}) do if E.equal(g(c,'id'),id) then return c end end end
local phrases=S.phrases
function M.model(scene,id)
  local c=named(scene,'classes',id); if not c then return nil end
  local ns=g(c,'ns') or G.name(g(c,'id'))
  local model={class=c,ns=ns,package=named(scene,'packages',g(c,'package')),title=g(g(scene,'diagram'),'title'),lang=g(c,'lang'),file=g(c,'file'),rels={}}
  for _,e in ipairs(g(scene,'edges') or {}) do
    local outgoing=E.equal(id,g(e,'from'))
    if outgoing or E.equal(id,g(e,'to')) then
      local other_id=g(e,outgoing and 'to' or 'from'); local other=named(scene,'classes',other_id)
      local p=phrases[G.name(g(e,'kind'))] or S.phrase_fallback
      model.rels[#model.rels+1]={id=other_id,name=g(other,'name') or G.name(other_id),kind=g(e,'kind'),label=g(e,'label'),['outgoing?']=outgoing,phrase=p[outgoing and 1 or 2]}
    end
  end
  return model
end
-- Merge the visible scene with the full overlaid document for independent
-- detail windows; relationships retain their leaf identities across namespaces.
function M.for_state(state,id)
  local doc=state.document
  if not doc then return M.model(state.scene,id) end
  if state.snapshots then doc=require('uml.application.overlay').apply(doc,state.snapshots) end
  local classes,edges=E.vector(),g(doc,'edges') or E.vector()
  local seen={}
  local function add(c)
    local cid=g(c,'id'); local key=E.encode(cid)
    if not seen[key] then seen[key]=true; classes[#classes+1]=c end
    for _,child in ipairs(g(c,'contents') or {}) do add(child) end
  end
  -- Authored leaf data wins over the abbreviated contents rows of collapsed
  -- namespace boxes. Then add root/current boxes for synthetic groups and ports.
  for _,c in ipairs(g(doc,'classes') or {}) do add(c) end
  for _,p in ipairs(g(doc,'packages') or {}) do for _,c in ipairs(g(p,'classes') or {}) do add(c) end end
  if g(doc,'hierarchical') then
    local root=require('uml.domain.hierarchy').view_at(doc,E.vector())
    for _,p in ipairs(g(root,'packages') or {}) do for _,c in ipairs(g(p,'classes') or {}) do add(c) end end
  end
  for _,c in ipairs(g(state.scene,'classes') or {}) do add(c) end
  return M.model(G.map{classes=classes,edges=edges,packages=g(state.scene,'packages') or E.vector(),diagram=g(state.scene,'diagram')},id)
end
function M.member_ident(model,member)
  local r={ns=g(model,'ns'),lang=g(model,'lang'),file=g(model,'file')}
  if member~=nil and tostring(member)~='' then r.name=tostring(member) end; return r
end
local function mu(c) if type(c)=='number' then return c end; return g(c,'mu') end
local function num(n) if n~=nil then return string.format('%.1f',n) end end
local function coverage(n) if n~=nil then return string.format('%.0f%%',100*n) end end
local function cells(c,class)
  local function count(k)
    local n=g(c,k); if n~=nil or not class then return n end
    for _,op in ipairs(g(c,'ops') or {}) do local v=g(op,k); if v~=nil then n=(n or 0)+v end end; return n
  end
  local crap=mu(g(c,'crap')); local cov=g(c,'coverage'); local killed,survived,uncovered,sites=count('killed'),count('survived'),count('uncovered'),count('sites')
  local r={['crap-s']=crap and (num(crap)..(class and 'μ' or '')),['crap-n']=crap,['cc-s']=not class and g(c,'cc') and tostring(math.floor(g(c,'cc'))) or nil,['cov-s']=coverage(cov),coverage=cov,killed=killed,survived=survived,uncovered=uncovered,sites=sites}
  if (killed or 0)+(survived or 0)+(uncovered or 0)==0 and (sites==nil or sites==0) then r['mut-note']=S.no_mutation_sites
  else for _,k in ipairs{'killed','survived','uncovered'} do if r[k]~=nil then r[k..'-s']=tostring(math.floor(r[k])) end end end
  return r
end
function M.rows(model)
  if not model then return nil end
  local c=g(model,'class'); local rows,y={},0
  local function emit(kind,text,extra) local r=extra or {}; r.kind=kind; r.text=text; r.y=y; r.h=r.h or 18; rows[#rows+1]=r; y=y+r.h end
  local function heading(label) y=y+12; emit('heading',label) end
  emit('name',g(c,'name'),{h=20})
  if g(c,'level')~=nil then emit('muted',S.level..g(c,'level')) end
  if g(c,'stereotype') then emit('muted','«'..G.name(g(c,'stereotype'))..'»') end
  local file=g(model,'file'); local module=g(model,'ns'); local base=file and file:gsub('\\','/'):match('[^/]+$')
  if base and not g(c,'drill?') and base:lower():match('%.rs$') then module=base end
  if module then emit('module',module,{module=true}) end
  emit('muted',S.package..(g(g(model,'package'),'label') or G.name(g(c,'package')) or ''))
  if g(model,'title') then emit('muted',g(model,'title')) end
  local crap=g(c,'crap')
  if crap then
    local text=type(crap)=='number' and S.crap_single:format(num(crap)) or S.crap_line:format(num(g(crap,'mu')) or '',num(g(crap,'max')) or '',num(g(crap,'sigma')) or '')
    emit('crap',text)
  end
  if #(g(c,'ops') or {})>0 or mu(crap)~=nil or g(c,'coverage')~=nil or g(c,'cc')~=nil or g(c,'killed')~=nil or g(c,'survived')~=nil then
    y=y+6; emit('group-header',''); emit('col-header',''); emit('stats',g(c,'name'),cells(c,true))
    for _,op in ipairs(g(c,'ops') or {}) do local r=cells(op,false); r.private=g(op,'private')==true; r['op-name']=g(op,'name'); emit('stats',(r.private and '- ' or '+ ')..(g(op,'text') or ''),r) end
  end
  if #(g(c,'fields') or {})>0 then heading(S.fields); for _,f in ipairs(g(c,'fields')) do emit('field',g(f,'text')) end end
  if #(g(model,'rels') or {})>0 then heading(S.relationships); for _,r in ipairs(g(model,'rels')) do emit('rel',r.phrase..'  '..r.name..(r.label and '  «'..r.label..'»' or ''),{id=r.id}) end end
  return rows
end
function M.content_h(rows) local last=rows and rows[#rows]; return last and last.y+last.h+16 or 32 end
-- The card is translated by a clamped content-space offset. One wheel notch
-- moves one step; the offset never leaves [0, content_h - view_h].
M.scroll_step=48
function M.max_scroll(rows,view_h)
  return math.max(0,M.content_h(rows)-(view_h or 0))
end
function M.scroll(rows,view_h,offset,delta)
  local maximum=M.max_scroll(rows,view_h)
  return math.max(0,math.min(maximum,(offset or 0)+(delta or 0)*M.scroll_step))
end
-- Hovered row state mirrors the draw contract: an operation name, or the
-- :module keyword for the source-identity row.
function M.highlight(rows,hover)
  if hover==nil then return nil end
  local module=E.kind(hover)=='keyword' and G.name(hover)=='module'
  for _,r in ipairs(rows or {}) do
    if module then if r.module then return r end
    elseif r['op-name']~=nil and r['op-name']==hover then return r end
  end
end
local function at(rows,y,key) for _,r in ipairs(rows or {}) do if r[key] and y>=r.y and y<=r.y+r.h-1 then return r[key] end end end
function M.rel_at(rows,y) for _,r in ipairs(rows or {}) do if r.kind=='rel' and y>=r.y and y<=r.y+r.h-1 then return r.id end end end
function M.member_at(rows,y) return at(rows,y,'op-name') end
function M.module_at(rows,y) return at(rows,y,'module')==true end
local columns={{id='crap',label=S.columns.crap,key='crap-s',w=56,group='crap'},{id='cc',label=S.columns.cc,key='cc-s',w=32,group='crap'},{id='cov',label=S.columns.cov,key='cov-s',w=44,group='crap'},{id='killed',label=S.columns.killed,key='killed-s',w=52,group='mutation'},{id='survived',label=S.columns.survived,key='survived-s',w=60,group='mutation'},{id='uncovered',label=S.columns.uncovered,key='uncovered-s',w=78,group='mutation'}}
function M.column_layout(width)
  local out,x={},math.max(560,width or 624)
  for i=#columns,1,-1 do local c={}; for k,v in pairs(columns[i]) do c[k]=v end; c.right=x; c.left=x-c.w; out[i]=c; x=c.left-10 end
  return out
end
function M.group_layout(width)
  local cs=M.column_layout(width); return {{id='crap',label=S.group_crap,left=cs[1].left,right=cs[3].right},{id='mutation',label=S.group_mutation,left=cs[4].left,right=cs[6].right}}
end
return M
