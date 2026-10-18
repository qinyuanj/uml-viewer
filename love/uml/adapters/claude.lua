-- Companion executable discovery at the native process boundary. Only a
-- native .exe or a .ps1 adapter is accepted: .cmd/.bat cannot preserve argv.
local Process=require('uml.adapters.process')
local Text=require('uml.domain.text')
local M={}
function M.executable(options)
  options=options or {}
  local run=options.run or Process.run
  if not Process.windows() then
    local named=os.getenv('CLAUDE_BIN')
    if named and named~='' then return named end
    return 'claude'
  end
  local script="$ErrorActionPreference='Stop';$name=if($env:CLAUDE_BIN){$env:CLAUDE_BIN}else{'claude'};"
    .."$c=Get-Command -Name $name -CommandType Application,ExternalScript -ErrorAction Stop|Select-Object -First 1;"
    .."if([IO.Path]::GetExtension($c.Source).ToLowerInvariant() -notin @('.exe','.ps1')){"
    .."throw 'Unsafe Claude wrapper. Set CLAUDE_BIN to native claude.exe (https://code.claude.com/docs/en/setup), "
    .."or a .ps1 adapter invoking node.exe and the official @anthropic-ai/claude-code/cli.js; .cmd/.bat cannot preserve arbitrary argv.'};"
    .."[Console]::Write($c.Source)"
  local result=run({'pwsh.exe','-NoProfile','-EncodedCommand',Text.encoded(script)})
  if result.exit~=0 then error('CLAUDE_BIN or Claude on PATH is required: '..tostring(result.err),0) end
  local path=(result.out or ''):gsub('%s+$','')
  if path=='' then error('CLAUDE_BIN or Claude on PATH is required',0) end
  return path
end
return M
