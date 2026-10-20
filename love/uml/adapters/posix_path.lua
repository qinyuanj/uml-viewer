-- POSIX 路径身份与目录枚举；拒绝扫描符号链接，避免越界和目录循环。
local ffi=require('ffi')
local lfs=require('lfs')
ffi.cdef[[char *realpath(const char *path, char *resolved_path); void free(void *ptr);]]
local M={}
function M.canonical(path)
  assert(type(path)=='string' and not path:find('\0',1,true),'invalid path')
  local resolved=ffi.C.realpath(path,nil)
  assert(resolved~=nil,'cannot canonicalize path: '..path)
  local result=ffi.string(resolved); ffi.C.free(resolved); return result
end
function M.entries(path)
  local result={}
  for name in lfs.dir(path) do
    if name~='.' and name~='..' then
      local mode,message=lfs.symlinkattributes(path..'/'..name,'mode')
      assert(mode,message)
      assert(mode~='link','source symbolic link unsupported: '..path..'/'..name)
      result[#result+1]={name=name,directory=mode=='directory'}
    end
  end
  table.sort(result,function(a,b) return a.name<b.name end); return result
end
return M
