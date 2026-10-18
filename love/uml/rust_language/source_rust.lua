local Source=require('uml.source')
-- Alternatives inside a group share regex priority and choose the earliest match.
local public={'pub%s+','pub%([^)]*%)%s+'}
local attribute='#%[%s*tauri%s*::%s*command[^%]]*%]%s*'
local attributed={attribute..'fn%s+@',attribute..'async%s+fn%s+@'}
for _,prefix in ipairs(public) do
  attributed[#attributed+1]=attribute..prefix..'fn%s+@'
  attributed[#attributed+1]=attribute..prefix..'async%s+fn%s+@'
end
local async_public,fn_public,types_public,types={},{},{},{}
for _,prefix in ipairs(public) do
  async_public[#async_public+1]=prefix..'async%s+fn%s+@'
  fn_public[#fn_public+1]=prefix..'fn%s+@'
  for _,kind in ipairs({'struct','enum','trait','type','const','static'}) do types_public[#types_public+1]=prefix..kind..'%s+@' end
end
for _,kind in ipairs({'struct','enum','trait','type','const','static'}) do types[#types+1]=kind..'%s+@' end
return Source.file_source({attributed,async_public,fn_public,'async%s+fn%s+@','fn%s+@',types_public,types},true)
