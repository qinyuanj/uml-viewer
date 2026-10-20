-- Companion executable discovery at the native process boundary. The entry is
-- exec'd directly, never through a shell or a script wrapper.
local M={}
function M.executable(options)
  options=options or {}
  local named=options.env and options.env.CLAUDE_BIN or os.getenv('CLAUDE_BIN')
  if named and named~='' then return named end
  return 'claude'
end
return M
