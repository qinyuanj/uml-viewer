-- OS display scale boundary. On Windows LOVE 11.5 starts DPI-unaware (the OS
-- bitmap-stretches the window and reports 96 DPI) so the viewer declares
-- per-monitor awareness before the window exists and reads the window DPI.
-- On POSIX the compositor (WSLg/Weston, macOS) owns scaling; LOVE's own
-- framebuffer scale already reports it, so no OS query is needed or possible.
-- UML_VIEWER_TEST_DPI_SCALE simulates a display scale in isolated native tests.
local M={}
local okffi,ffi=pcall(require,'ffi')
local user32
local function windows()
  if not okffi or ffi.os~='Windows' then return nil end
  if user32==nil then
    local ok=pcall(ffi.cdef,[[
      int SetProcessDpiAwarenessContext(void* context);
      void* GetActiveWindow(void);
      unsigned int GetDpiForWindow(void* window);
      unsigned int GetDpiForSystem(void);
    ]])
    local loaded,lib=pcall(ffi.load,'user32')
    user32=(ok and loaded) and lib or false
  end
  return user32 or nil
end
-- DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2 is the pseudo-handle -4. Returns
-- true when the process is now (or already was) per-monitor aware.
function M.enable_awareness()
  local u=windows(); if not u then return false end
  local ok,result=pcall(function() return u.SetProcessDpiAwarenessContext(ffi.cast('void*',-4)) end)
  return ok and result~=0
end
function M.override()
  local v=tonumber(os.getenv('UML_VIEWER_TEST_DPI_SCALE') or '')
  if v and v>0 then return v end
end
-- OS display scale (1 = 96 DPI) of the active window, else the system DPI.
-- nil means "the backend already reports the real scale".
function M.os_scale()
  local forced=M.override(); if forced then return forced end
  local u=windows(); if not u then return nil end
  local ok,dpi=pcall(function()
    local window=u.GetActiveWindow()
    local value=window~=nil and u.GetDpiForWindow(window) or 0
    if value==0 then value=u.GetDpiForSystem() end
    return tonumber(value)
  end)
  if ok and dpi and dpi>0 then return dpi/96 end
end
return M
