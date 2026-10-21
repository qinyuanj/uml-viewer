-- Deterministic synthetic IR for the viewer stack benchmark (map #124).
-- Usage: lua5.4 bench/gen_synthetic.lua <classes> <packages> <out.edn> [chain]
-- Same arguments give byte-identical output under Lua 5.4 and LuaJIT.
-- "chain" reproduces the #87 baseline scene: 500 flat name-only classes in one
-- dependency chain, no quality data. It survives cimgui's 16-bit vertex index
-- ceiling where the realistic scenes abort; keep it for the legacy reference.
local N=assert(tonumber(arg[1]),'classes')
local P=assert(tonumber(arg[2]),'packages')
local out_path=assert(arg[3],'output path')
if arg[4]=='chain' then
  local out={'; legacy chain scene ('..N..' classes)\n{:hierarchical true :title "Synthetic '..N..'" :classes ['}
  for i=1,N do
    out[#out+1]=string.format('{:id :class%d :name "Class%d" :ns "synthetic.class%d"} ',i,i,i)
  end
  out[#out+1]=']:edges ['
  for i=1,N-1 do
    out[#out+1]=string.format('{:from :class%d :to :class%d :kind :dependency} ',i,i+1)
  end
  out[#out+1]=']}\n'
  local f=assert(io.open(out_path,'wb')); f:write(table.concat(out)); f:close()
  io.stdout:write(string.format('%s: chain, %d classes, %d edges\n',out_path,N,N-1))
  os.exit(0)
end
-- members p50 3 / p90 13 / max 34, class name length p50 7 / p90 13,
-- member text length p50 10 / p90 17, about 3 dependency edges per class.
local N=assert(tonumber(arg[1]),'classes')
local P=assert(tonumber(arg[2]),'packages')
local out_path=assert(arg[3],'output path')

-- Park-Miller minimal standard generator; exact in doubles on both runtimes.
local seed=20261021+N
local function rand() seed=(seed*48271)%2147483647; return seed/2147483647 end
local function int(a,b) return a+math.floor(rand()*(b-a+1)) end
-- Piecewise-linear inverse CDF through {quantile,value} knots.
local function sample(knots)
  local u=rand()
  for i=2,#knots do
    local q0,v0=knots[i-1][1],knots[i-1][2]; local q1,v1=knots[i][1],knots[i][2]
    if u<=q1 then return math.floor(v0+(v1-v0)*(u-q0)/(q1-q0)+.5) end
  end
  return knots[#knots][2]
end
local MEMBERS={{0,0},{.2,1},{.5,3},{.9,13},{.98,24},{1,34}}
local NAME_LEN={{0,3},{.5,7},{.9,13},{1,19}}
local TEXT_LEN={{0,4},{.5,10},{.9,17},{1,25}}

local SYL={'ab','ac','ad','al','an','ar','as','at','ba','be','bo','ca','ce','co','da','de','di','do','el','en','er','es',
  'fa','fi','fo','ga','ge','go','ha','he','in','is','it','ka','la','le','li','lo','ma','me','mi','mo','na','ne','no',
  'or','pa','pe','po','ra','re','ri','ro','sa','se','si','so','ta','te','ti','to','un','ur','va','ve','vi','wa','ze'}
local CJK={'图','层','包','类','边','视','口','缩','放','质','量','快','照','邮','件','队','列','提','案','源','码','检','查','器'}
local function word(n)
  local s={} ; local len=0
  while len<n do local y=SYL[int(1,#SYL)]; s[#s+1]=y; len=len+#y end
  return table.concat(s):sub(1,n)
end
local function camel(n) local w=word(n); return w:sub(1,1):upper()..w:sub(2) end
-- 5% of class names are CJK: a deliberate glyph-path stress (on-demand CJK
-- glyphs), not a claim that Lua modules are named this way.
local function class_name()
  if rand()<.05 then local s={}; for _=1,int(2,6) do s[#s+1]=CJK[int(1,#CJK)] end; return table.concat(s) end
  return camel(math.max(3,sample(NAME_LEN)))
end
local function member_text()
  local n=math.max(4,sample(TEXT_LEN))
  if rand()<.3 then return word(math.max(2,n-2))..' : '..camel(3) end -- field-like
  return 'M.'..word(math.max(2,n-2))
end
local function str(s) return '"'..s:gsub('\\','\\\\'):gsub('"','\\"')..'"' end
local function fmt(x) if x==math.floor(x) then return string.format('%d',x) end; return string.format('%.1f',x) end

local packages={}
local classes={}
for p=1,P do
  local label=string.format('synthetic.%s.p%02d',word(int(4,8)),p)
  packages[p]={id=string.format('p%02d',p),label=label,classes={}}
end
for i=1,N do
  local p=((i-1)%P)+1
  local c={id=string.format('c%04d',i),name=class_name(),package=p,level=int(1,4),ops={},fields={}}
  for _=1,sample(MEMBERS) do
    local t=member_text()
    if t:find(' : ',1,true) then c.fields[#c.fields+1]=t else c.ops[#c.ops+1]=t end
  end
  -- Quality mix: 10% no data at all (red, dashed, 无数据), 15% CRAP only,
  -- 75% both. CRAP mu skews low with a tail; mutation ratio is mostly high.
  local q=rand()
  if q>=.10 then
    local mu=math.floor((2+28*rand()^2.5)*10+.5)/10
    c.crap={mu=mu,max=mu+math.floor(rand()*80)/10,sigma=math.floor(rand()*40)/10,n=math.max(1,#c.ops)}
  end
  if q>=.25 then
    local sites=int(4,60); local killed=math.floor(sites*(.55+.45*rand()^.5)+.5)
    c.killed=killed; c.survived=sites-killed
  end
  classes[i]=c
  local pk=packages[p].classes; pk[#pk+1]=c
end
-- About three edges per class: 80% stay in the package, 20% cross it.
-- 4% are inheritance, 3% flagged violating.
local edges,seen={},{}
for i=1,N do
  local from=classes[i]
  for _=1,sample{{0,1},{.5,3},{.9,5},{1,8}} do
    local to
    if rand()<.8 then local pk=packages[from.package].classes; to=pk[int(1,#pk)]
    else to=classes[int(1,N)] end
    local key=from.id..'>'..to.id
    if to~=from and not seen[key] then
      seen[key]=true
      local r=rand()
      edges[#edges+1]={from=from.id,to=to.id,kind=r<.04 and 'inheritance' or 'dependency',violating=rand()<.03}
    end
  end
end

local out={}
local function w(s) out[#out+1]=s end
w(string.format(';; Generated by bench/gen_synthetic.lua %d %d. Do not edit.\n',N,P))
w(string.format('{:title %s\n :direction :tb\n :packages [',str(string.format('Synthetic %d',N))))
for pi,p in ipairs(packages) do
  w(string.format('%s{:id :%s :label %s :classes [',pi>1 and '\n   ' or '',p.id,str(p.label)))
  for ci,c in ipairs(p.classes) do
    local parts={string.format(':id :%s :name %s :ns %s :level %d',c.id,str(c.name),str(p.label..'.'..c.id),c.level)}
    if c.crap then parts[#parts+1]=string.format(':crap {:mu %s :max %s :sigma %s :n %d}',fmt(c.crap.mu),fmt(c.crap.max),fmt(c.crap.sigma),c.crap.n) end
    if c.killed then parts[#parts+1]=string.format(':killed %d :survived %d',c.killed,c.survived) end
    for _,kind in ipairs{'fields','ops'} do
      if #c[kind]>0 then local xs={}; for j,t in ipairs(c[kind]) do xs[j]=string.format('{:text %s}',str(t)) end; parts[#parts+1]=':'..kind..' ['..table.concat(xs,' ')..']' end
    end
    w((ci>1 and '\n    ' or '\n    ')..'{'..table.concat(parts,' ')..'}')
  end
  w(']}')
end
w(']\n :edges [')
for i,e in ipairs(edges) do
  w(string.format('%s{:from :%s :to :%s :kind :%s%s}',i>1 and '\n  ' or '',e.from,e.to,e.kind,e.violating and ' :violating true' or ''))
end
w(']}\n')
local f=assert(io.open(out_path,'wb')); f:write(table.concat(out)); f:close()
io.stdout:write(string.format('%s: %d classes, %d packages, %d edges\n',out_path,N,P,#edges))
