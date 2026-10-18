-- Native Windows path identity; UTF-8 API boundary, canonical handles internally.
local ffi=require('ffi')
ffi.cdef[[
int MultiByteToWideChar(unsigned int,unsigned long,const char*,int,wchar_t*,int);
int WideCharToMultiByte(unsigned int,unsigned long,const wchar_t*,int,char*,int,const char*,int*);
void* CreateFileW(const wchar_t*,unsigned long,unsigned long,void*,unsigned long,unsigned long,void*);
unsigned long GetFinalPathNameByHandleW(void*,wchar_t*,unsigned long,unsigned long);
int CloseHandle(void*);
typedef struct { unsigned long attributes; unsigned long times[6]; unsigned long high; unsigned long low; unsigned long reserved[2]; wchar_t name[260]; wchar_t alternate[14]; } UML_FIND_DATA;
void* FindFirstFileW(const wchar_t*,UML_FIND_DATA*);
int FindNextFileW(void*,UML_FIND_DATA*);
int FindClose(void*);
unsigned long GetLastError(void);
]]
local M={}
function M.canonical(path)
  local n=ffi.C.MultiByteToWideChar(65001,8,path,-1,nil,0); assert(n>0,'invalid UTF-8 path')
  local wide=ffi.new('wchar_t[?]',n); assert(ffi.C.MultiByteToWideChar(65001,8,path,-1,wide,n)>0)
  local handle=ffi.C.CreateFileW(wide,0,7,nil,3,0x02000000,nil)
  assert(handle~=ffi.cast('void*',-1),'path does not exist: '..path)
  local size=ffi.C.GetFinalPathNameByHandleW(handle,nil,0,0)
  if size==0 then ffi.C.CloseHandle(handle); error('cannot canonicalize path: '..path) end
  local buf=ffi.new('wchar_t[?]',size+1); local used=ffi.C.GetFinalPathNameByHandleW(handle,buf,size+1,0); ffi.C.CloseHandle(handle)
  assert(used>0 and used<=size,'cannot canonicalize path: '..path)
  local count=ffi.C.WideCharToMultiByte(65001,0,buf,used,nil,0,nil,nil); local utf=ffi.new('char[?]',count)
  assert(ffi.C.WideCharToMultiByte(65001,0,buf,used,utf,count,nil,nil)>0)
  local result=ffi.string(utf,count):gsub('\\','/')
  if result:sub(1,8)=='//?/UNC/' then return '//'..result:sub(9) end
  return result:gsub('^//%?/',''):gsub('/$','')
end
function M.entries(path)
  local pattern=path..'/*'; local n=ffi.C.MultiByteToWideChar(65001,8,pattern,-1,nil,0); local wide=ffi.new('wchar_t[?]',n)
  assert(ffi.C.MultiByteToWideChar(65001,8,pattern,-1,wide,n)>0)
  local data=ffi.new('UML_FIND_DATA[1]'); local handle=ffi.C.FindFirstFileW(wide,data)
  assert(handle~=ffi.cast('void*',-1),'cannot enumerate directory: '..path)
  local result={}
  repeat
    local count=ffi.C.WideCharToMultiByte(65001,0,data[0].name,-1,nil,0,nil,nil); local text=ffi.new('char[?]',count)
    ffi.C.WideCharToMultiByte(65001,0,data[0].name,-1,text,count,nil,nil); local name=ffi.string(text)
    if name~='.' and name~='..' then
      -- Reparse points are refused rather than following directory cycles.
      local attr=tonumber(data[0].attributes)
      assert(math.floor(attr/1024)%2==0,'source reparse point unsupported: '..path..'/'..name)
      result[#result+1]={name=name,directory=math.floor(attr/16)%2==1}
    end
  until ffi.C.FindNextFileW(handle,data)==0
  local code=ffi.C.GetLastError(); ffi.C.FindClose(handle); assert(code==18,'directory enumeration failed: '..path)
  table.sort(result,function(a,b) return a.name<b.name end); return result
end
return M
