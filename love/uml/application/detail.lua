-- Public detail card model and content-space rows; no rendering dependencies.
local E=require('uml.edn')
local G=require('uml.domain.geom')
local M={width=640,height=700,pad=16,col_gap=10}
local function g(t,k) if E.kind(t)=='map' then return G.get(t,k) end; return t and t[k] end
local function named(scene,key,id) for _,c in ipairs(g(scene,key) or {}) do if E.equal(g(c,'id'),id) then return c end end end
local phrases={inheritance={'extends','extended by'},implements={'implements','implemented by'},association={'associates with','associated from'},dependency={'depends on','used by'},aggregation={'aggregates','aggregated by'},composition={'composes','composed in'}}
function M.model(scene,id)
  local c=named(scene,'classes',id); if not c then return nil end
  local ns=g(c,'ns') or G.name(g(c,'id'))
  local model={class=c,ns=ns,package=named(scene,'packages',g(c,'package')),title=g(g(scene,'diagram'),'title'),lang=g(c,'lang'),file=g(c,'file'),rels={}}
  for _,e in ipairs(g(scene,'edges') or {}) do
    local outgoing=E.equal(id,g(e,'from'))
    if outgoing or E.equal(id,g(e,'to')) then
      local other_id=g(e,outgoing and 'to' or 'from'); local other=named(scene,'classes',other_id)
      local p=phrases[G.name(g(e,'kind'))] or {'to','from'}
      model.rels[#model.rels+1]={id=other_id,name=g(other,'name') or G.name(other_id),kind=g(e,'kind'),label=g(e,'label'),['outgoing?']=outgoing,phrase=p[outgoing and 1 or 2]}
    end
  end
  return model
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
  if (killed or 0)+(survived or 0)+(uncovered or 0)==0 and (sites==nil or sites==0) then r['mut-note']='---no mutation sites---'
  else for _,k in ipairs{'killed','survived','uncovered'} do if r[k]~=nil then r[k..'-s']=tostring(math.floor(r[k])) end end end
  return r
end
function M.rows(model)
  if not model then return nil end
  local c=g(model,'class'); local rows,y={},16
  local function emit(kind,text,extra) local r=extra or {}; r.kind=kind; r.text=text; r.y=y; r.h=r.h or 18; rows[#rows+1]=r; y=y+r.h end
  local function heading(label) y=y+12; emit('heading',label) end
  emit('name',g(c,'name'))
  if g(c,'level')~=nil then emit('muted','Level '..g(c,'level')) end
  if g(c,'stereotype') then emit('muted','«'..G.name(g(c,'stereotype'))..'»') end
  local file=g(model,'file'); local module=g(model,'ns'); local base=file and file:gsub('\\','/'):match('[^/]+$')
  if base and not g(c,'drill?') and base:lower():match('%.rs$') then module=base end
  if module then emit('module',module,{module=true}) end
  emit('muted','package  '..(g(g(model,'package'),'label') or G.name(g(c,'package')) or ''))
  if g(model,'title') then emit('muted',g(model,'title')) end
  local crap=g(c,'crap')
  if crap then
    local text=type(crap)=='number' and ('Crap '..num(crap)) or ('Crap μ '..(num(g(crap,'mu')) or '')..' max '..(num(g(crap,'max')) or '')..' σ '..(num(g(crap,'sigma')) or ''))
    emit('crap',text)
  end
  if #(g(c,'ops') or {})>0 or mu(crap)~=nil or g(c,'coverage')~=nil or g(c,'cc')~=nil or g(c,'killed')~=nil or g(c,'survived')~=nil then
    y=y+12; emit('group-header',''); emit('col-header',''); emit('stats',g(c,'name'),cells(c,true))
    for _,op in ipairs(g(c,'ops') or {}) do local r=cells(op,false); r.private=g(op,'private')==true; r['op-name']=g(op,'name'); emit('stats',(r.private and '- ' or '+ ')..(g(op,'text') or ''),r) end
  end
  if #(g(c,'fields') or {})>0 then heading('Fields'); for _,f in ipairs(g(c,'fields')) do emit('field',g(f,'text')) end end
  if #(g(model,'rels') or {})>0 then heading('Relationships'); for _,r in ipairs(g(model,'rels')) do emit('rel',r.phrase..'  '..r.name..(r.label and '  «'..r.label..'»' or ''),{id=r.id}) end end
  return rows
end
function M.content_h(rows) local last=rows and rows[#rows]; return last and last.y+last.h+16 or 32 end
local function at(rows,y,key) for _,r in ipairs(rows or {}) do if r[key] and y>=r.y and y<=r.y+r.h-1 then return r[key] end end end
function M.rel_at(rows,y) for _,r in ipairs(rows or {}) do if r.kind=='rel' and y>=r.y and y<=r.y+r.h-1 then return r.id end end end
function M.member_at(rows,y) return at(rows,y,'op-name') end
function M.module_at(rows,y) return at(rows,y,'module')==true end
local columns={{id='crap',label='Crap',key='crap-s',w=56,group='crap'},{id='cc',label='CC',key='cc-s',w=32,group='crap'},{id='cov',label='Cov',key='cov-s',w=44,group='crap'},{id='killed',label='killed',key='killed-s',w=52,group='mutation'},{id='survived',label='survived',key='survived-s',w=60,group='mutation'},{id='uncovered',label='uncovered',key='uncovered-s',w=78,group='mutation'}}
function M.column_layout()
  local out,x={},624
  for i=#columns,1,-1 do local c={}; for k,v in pairs(columns[i]) do c[k]=v end; c.right=x; c.left=x-c.w; out[i]=c; x=c.left-10 end
  return out
end
function M.group_layout()
  local cs=M.column_layout(); return {{id='crap',label='--crap--',left=cs[1].left,right=cs[3].right},{id='mutation',label='--mutation--',left=cs[4].left,right=cs[6].right}}
end
return M
