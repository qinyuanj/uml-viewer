-- Native file timestamp boundary; compare full FILETIME words without rounding.
local ffi=require('ffi')
ffi.cdef[[int GetFileAttributesExW(const wchar_t*,int,void*);]]
local M={}
function M.mtime(path)
  local n=ffi.C.MultiByteToWideChar(65001,8,path,-1,nil,0); assert(n>0,'invalid UTF-8 path')
  local wide=ffi.new('wchar_t[?]',n); ffi.C.MultiByteToWideChar(65001,8,path,-1,wide,n)
  local data=ffi.new('uint32_t[9]'); assert(ffi.C.GetFileAttributesExW(wide,0,data)~=0,'cannot stat '..path)
  return tostring(data[5])..':'..tostring(data[6])
end
return M
