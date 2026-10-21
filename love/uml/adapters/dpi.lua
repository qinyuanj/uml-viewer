-- OS display scale boundary. On WSL the compositor (WSLg/Weston) owns
-- scaling; LOVE's own framebuffer scale already reports it, so there is no OS
-- query to make. UML_VIEWER_TEST_DPI_SCALE simulates a display scale in
-- isolated native tests.
local M={}
-- Nothing to declare: no per-monitor DPI awareness API exists under WSLg.
-- Kept as a named seam so conf.lua does not reach for an OS query.
function M.enable_awareness() return false end
function M.override()
  local v=tonumber(os.getenv('UML_VIEWER_TEST_DPI_SCALE') or '')
  if v and v>0 then return v end
end
-- OS display scale (1 = 96 DPI); nil means "the backend already reports the
-- real scale", which it does under WSLg.
function M.os_scale()
  return M.override()
end
return M
