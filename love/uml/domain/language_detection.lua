-- Pure language selection from project-relative filename snapshots.
local E=require('uml.edn')
local M={}
local function get(p,k) local v=E.get(p,E.keyword(k)); if v~=E.nil_value and v~=false then return v end end
local function supported(lang)
  local name=E.kind(lang)=='keyword' and lang.name or lang
  assert(name~='clojure','Clojure is not supported by the LOVE implementation')
  return lang
end
function M.select(policy,filenames)
  local lang=supported(get(policy,'lang'))
  local sources=get(policy,'sources')
  if sources then
    assert(#sources>0,':sources must contain at least one source with explicit :lang')
    for _,source in ipairs(sources) do assert(supported(get(source,'lang') or lang),'Each :sources entry requires explicit :lang') end
    return nil
  end
  if lang then return lang end
  local found={}
  for _,path in ipairs(filenames or {}) do
    path=path:gsub('\\','/'):gsub('^%./','')
    if path=='package.json' then found.typescript=true
    elseif path=='Cargo.toml' then found.rust=true
    elseif path=='pyproject.toml' then found.python=true
    elseif path:match('%.lua$') then found.lua=true end
  end
  local languages={}; for _,lang in ipairs{'typescript','rust','python','lua'} do if found[lang] then languages[#languages+1]=lang end end
  assert(#languages==1,'Cannot detect one language; declare explicit :lang or :sources')
  return E.keyword(languages[1])
end
return M
