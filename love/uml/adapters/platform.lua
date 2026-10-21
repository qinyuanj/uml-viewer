-- The single delivered-platform decision (ADR 0007): WSL2 + WSLg, which LOVE
-- reports as Linux. Callers receive only whether the host is supported and,
-- if not, why; no other module compares host names.
local M={}

function M.support(system)
  system=system or (love and love.system)
  local name=system and system.getOS() or 'Linux'
  if name=='Linux' then return {supported=true} end
  if name:sub(1,7)=='Windows' then
    return {supported=false,reason='Windows native is not a supported platform (ADR 0007). Use WSL2 + WSLg'}
  end
  return {supported=false,reason='Unsupported companion platform: '..name..'. Run the viewer and Herdr natively'}
end

return M
