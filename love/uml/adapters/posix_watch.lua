-- POSIX nanosecond mtime stamp via statx. lfs only reports whole seconds, so
-- two edits inside one second would look unchanged and never reload. The
-- stamp is opaque: consumers compare it for equality, never parse it.
local ffi=require('ffi')
ffi.cdef[[
typedef struct { long long tv_sec; unsigned int tv_nsec; int reserved; } uml_statx_timestamp;
typedef struct {
  unsigned int mask, blksize; unsigned long long attributes;
  unsigned int nlink, uid, gid; unsigned short mode, spare0;
  unsigned long long ino, size, blocks, attributes_mask;
  uml_statx_timestamp atime, btime, ctime, mtime;
  unsigned int rdev_major, rdev_minor, dev_major, dev_minor;
  unsigned long long mnt_id; unsigned int dio_mem_align, dio_offset_align;
  unsigned long long spare3[12];
} uml_statx;
int statx(int dirfd, const char *pathname, int flags, unsigned int mask, uml_statx *buf);
]]
local AT_FDCWD=-100
local STATX_MTIME=0x40
local STATX_SIZE=0x200
local M={}
function M.mtime(path)
  local buffer=ffi.new('uml_statx')
  local ok=ffi.C.statx(AT_FDCWD,path,0,STATX_MTIME+STATX_SIZE,buffer)
  assert(ok==0,'cannot stat '..path)
  return ('%d:%d:%d'):format(tonumber(buffer.mtime.tv_sec),tonumber(buffer.mtime.tv_nsec),tonumber(buffer.size))
end
return M
