-- Clojure 1.12.0 group-by's keyword-vector key enumeration, without a runtime
-- dependency. Source: clojure/clojure tag clojure-1.12.0, src/jvm/clojure/lang/
-- Murmur3.java (hashUnencodedChars/hashOrdered), Symbol.java, Keyword.java,
-- Util.java (hashCombine), PersistentArrayMap.java (8 entries), and
-- PersistentHashMap.java (mask/NodeSeq/ArrayNode.Seq/HashCollisionNode).
-- Only distinct directed keyword pairs are accepted, in first encounter order.
local M={}
local U=4294967296
local function xor(a,b)
  local result,place=0,1
  for _=1,32 do
    local x,y=a%2,b%2
    if x~=y then result=result+place end
    a,b,place=math.floor(a/2),math.floor(b/2),place*2
  end
  return result
end
local function mul(a,b)
  -- Split products to keep every intermediate exact in LuaJIT doubles.
  local al,bl=a%65536,b%65536
  return (al*bl+((math.floor(a/65536)*bl+al*math.floor(b/65536))%65536)*65536)%U
end
local function rot(a,n) return (a*2^n)%U+math.floor(a/2^(32-n)) end
local function mixk(k) return mul(rot(mul(k,3432918353),15),461845907) end
local function mixh(h,k) return (rot(xor(h,k),13)*5+3864292196)%U end
local function fmix(h,n)
  h=xor(h,n); h=xor(h,math.floor(h/65536)); h=mul(h,2246822507)
  h=xor(h,math.floor(h/8192)); h=mul(h,3266489909)
  return xor(h,math.floor(h/65536))
end
local function utf16(s)
  local out,i={},1
  while i<=#s do
    local a=s:byte(i); local cp,n=a,1
    if a>=240 then cp,n=a%8,4 elseif a>=224 then cp,n=a%16,3 elseif a>=192 then cp,n=a%32,2 end
    for j=1,n-1 do cp=cp*64+s:byte(i+j)%64 end
    if cp>65535 then cp=cp-65536; out[#out+1]=55296+math.floor(cp/1024); cp=56320+cp%1024 end
    out[#out+1]=cp; i=i+n
  end
  return out
end
local function string_hash(s)
  local units=utf16(s); local h=0
  for i=1,#units-1,2 do h=mixh(h,mixk(units[i]+units[i+1]*65536)) end
  if #units%2==1 then h=xor(h,mixk(units[#units])) end
  return fmix(h,#units*2)
end
local function keyword_hash(k)
  local ns,name=k.name:match('^(.-)/(.+)$')
  name=name or k.name
  local h=string_hash(name); local nh=0
  if ns then for _,c in ipairs(utf16(ns)) do nh=(31*nh+c)%U end end
  -- Java's signed >> in Util.hashCombine, unlike Murmur's unsigned >>>.
  local signed=h>=2147483648 and h-U or h
  return (xor(h,(nh+2654435769+(h*64)%U+math.floor(signed/4))%U)+2654435769)%U
end
local function pair_hash(pair)
  local h=(31+keyword_hash(pair[1]))%U
  h=(31*h+keyword_hash(pair[2]))%U
  return fmix(mixh(0,mixk(h)),2)
end
function M.indices(pairs)
  local out={}
  for i=1,#pairs do out[i]=i end
  -- PersistentArrayMap assoc updates preserve slots; the ninth distinct key
  -- promotes to a HAMT. Both bitmap and array nodes enumerate ascending slots,
  -- recursively, using low five-bit hash fragments first. Full hash collisions
  -- preserve insertion order (HashCollisionNode's entry array).
  if #pairs<=8 then return out end
  local hashes={}
  for i,pair in ipairs(pairs) do hashes[i]=pair_hash(pair) end
  table.sort(out,function(a,b)
    local x,y=hashes[a],hashes[b]
    for _=1,7 do
      if x%32~=y%32 then return x%32<y%32 end
      x,y=math.floor(x/32),math.floor(y/32)
    end
    return a<b
  end)
  return out
end
return M
