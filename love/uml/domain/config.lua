-- Quality values use the same tagged EDN boundary as policy and view state.
local E = require('uml.edn')
local M = {}
M.crap_thresholds = E.parse('{:green 8 :yellow 12 :red 20}')
M.grade_best, M.grade_mid, M.grade_worst = 10, 5.5, 1
function M.crap_band(score)
  return E.keyword(score == nil and 'red' or score <= 8 and 'green' or score <= 12 and 'yellow' or 'red')
end
local function get(m, key)
  if E.kind(m) == 'map' then local v = E.get(m, E.keyword(key)); if v ~= E.nil_value then return v end end
end
M.mutation_thresholds = E.parse('{:red 0.8 :yellow 0.9 :green 1}')
function M.mutation_band(score)
  return E.keyword(score == nil and 'red' or score <= 0.8 and 'red' or score <= 0.9 and 'yellow' or 'green')
end
local function along(x,a,b,c,y,z,w)
  if x <= a then return y elseif x <= b then return y+(x-a)/(b-a)*(z-y)
  elseif x <= c then return z+(x-b)/(c-b)*(w-z) else return w end
end
function M.crap_grade(score) if score == nil then return 1 end; return along(score,8,12,20,10,5.5,1) end
function M.mutation_grade(score) if score == nil then return 1 end; return along(score,0.8,0.9,1,1,5.5,10) end
function M.crap_risk(crap) local mu=get(crap,'mu'); if mu~=nil then return mu+(get(crap,'sigma') or 0) end end
function M.combined_grade(a,b) if a~=nil and b~=nil then return (a+b)/2 end; if a~=nil then return a end; return b end
function M.mutation_ratio(m)
  local k,s,g=get(m,'killed'),get(m,'survived'),get(m,'mut-gap')
  if k~=nil or s~=nil or (g and g>0) then local n=(k or 0)+(s or 0)+(g or 0); if n>0 then return (k or 0)/n end end
end
local function map(t) local m=E.map(); for k,v in pairs(t) do E.assoc(m,E.keyword(k),v) end; return m end
local function score(x) if type(x)=='number' then return x end; return get(x,'mu') end
local function group(xs)
  local sum,mx=0,-math.huge
  for _,x in ipairs(xs) do sum=sum+x; mx=math.max(mx,x) end
  local mu=sum/#xs; local variance=0
  for _,x in ipairs(xs) do variance=variance+(x-mu)^2 end
  return {n=#xs,mu=mu,sigma=math.sqrt(variance/#xs),max=mx}
end
local function rounded(g)
  local function r(x) return math.floor(x*10+0.5)/10 end
  return map{n=g.n,mu=r(g.mu),sigma=r(g.sigma),max=r(g.max)}
end
function M.function_weight(c) return math.max(1,#(get(c,'ops') or {})) end
function M.summarize_scores(scores)
  local xs={}; for _,x in ipairs(scores or {}) do local n=score(x); if n~=nil then xs[#xs+1]=n end end
  if #xs>0 then return rounded(group(xs)) end
end
local function module(c) return not get(c,'foreign') and not E.equal(get(c,'shape'),E.keyword('oval')) end
function M.pool_crap(classes,rolled)
  local groups={}
  for _,c in ipairs(classes or {}) do if module(c) then
    local xs={}; for _,op in ipairs(get(c,'ops') or {}) do local n=score(get(op,'crap')); if n~=nil then xs[#xs+1]=n end end
    local summary=get(c,'crap'); local mu=get(summary,'mu'); local g
    if #xs>0 and not (rolled and mu~=nil) then g=group(xs)
    elseif mu~=nil then
      local n=get(summary,'n'); if not n or n<=0 then n=#xs>0 and #xs or M.function_weight(c) end
      g={n=math.floor(n),mu=mu,sigma=get(summary,'sigma') or 0,max=get(summary,'max') or mu}
    else g={n=M.function_weight(c),mu=20,sigma=0,max=20} end
    groups[#groups+1]=g
  end end
  if #groups==0 then return nil end
  local n,sum,m2,mx=0,0,0,-math.huge
  for _,g in ipairs(groups) do n=n+g.n; sum=sum+g.n*g.mu; m2=m2+g.n*(g.sigma^2+g.mu^2); mx=math.max(mx,g.max) end
  local mu=sum/n; return rounded{n=n,mu=mu,sigma=math.sqrt(math.max(0,m2/n-mu^2)),max=mx}
end
function M.pool_mutants(classes)
  local k,s,g,u,measured,uncovered=0,0,0,0,false,false
  for _,c in ipairs(classes or {}) do if module(c) then
    local ck,cs,cg,cu=get(c,'killed'),get(c,'survived'),get(c,'mut-gap'),get(c,'uncovered')
    local has=ck~=nil or cs~=nil
    if has then measured=true; k=k+(ck or 0); s=s+(cs or 0) end
    if cu~=nil then uncovered=true; u=u+cu end
    g=g+((has or cg~=nil) and (cg or 0) or M.function_weight(c))
  end end
  if not measured and g<=0 then return nil end
  local r=E.map()
  if measured then E.assoc(r,E.keyword('killed'),k); E.assoc(r,E.keyword('survived'),s) end
  if uncovered then E.assoc(r,E.keyword('uncovered'),u) end
  if g>0 then E.assoc(r,E.keyword('mut-gap'),g) end
  return r
end
return M
