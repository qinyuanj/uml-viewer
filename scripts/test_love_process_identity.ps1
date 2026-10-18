# Native Windows check: the LOVE FFI adapter records the actual luajit process
# identity with full FILETIME precision, matching .NET's creation ticks exactly.
param(
    [string]$LuaJit = "$env:TEMP\uml-53-luajit-20261018\src\luajit.exe"
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
if (-not (Test-Path -LiteralPath $LuaJit)) { throw "LuaJIT not found: $LuaJit" }
$root = Join-Path ([IO.Path]::GetTempPath()) ('uml-69-identity-' + [guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory -Force -Path $root
$release = Join-Path $root 'release'
$driver = Join-Path $root 'driver.lua'
$rootForward = $root.Replace('\', '/')
$repoForward = $repo.Replace('\', '/')
$process = $null
try {
    $source = @"
package.path='$repoForward/love/?.lua;$repoForward/love/?/init.lua;'..package.path
local record,path=require('uml.adapters.process_identity').record('$rootForward')
io.write(record.pid, '\n')
while true do
  local f=io.open('$rootForward/release','rb')
  if f then f:close(); break end
  os.execute('ping -n 2 127.0.0.1 >NUL')
end
"@
    Set-Content -LiteralPath $driver -Value $source -Encoding utf8
    $process = Start-Process -FilePath $LuaJit -ArgumentList @($driver) -PassThru -WindowStyle Hidden
    $recordPath = Join-Path $root '.uml-viewer/viewer-process-identity.json'
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    while (-not (Test-Path -LiteralPath $recordPath) -and [DateTime]::UtcNow -lt $deadline) {
        if ($process.HasExited) { throw "identity writer exited early with $($process.ExitCode)" }
        Start-Sleep -Milliseconds 50
    }
    if (-not (Test-Path -LiteralPath $recordPath)) { throw 'identity writer did not publish a record' }
    $record = Get-Content -LiteralPath $recordPath -Raw | ConvertFrom-Json
    if ($record.runtime -cne 'love') { throw "unexpected runtime $($record.runtime)" }
    if ([int]$record.pid -ne $process.Id) {
        throw "recorded pid $($record.pid) does not match writer $($process.Id)"
    }
    if ([string]$record.cwd -cne $rootForward) { throw "unexpected cwd $($record.cwd)" }
    $actual = Get-Process -Id $process.Id
    $ticks = [bigint]$actual.StartTime.ToUniversalTime().Ticks - [bigint]504911232000000000
    if ([bigint]::Parse([string]$record.started) -ne $ticks) {
        throw "FILETIME mismatch: recorded $($record.started), actual $ticks"
    }
    Write-Output "LOVE identity verified: pid $($record.pid), FILETIME $($record.started)"
} finally {
    $null = New-Item -ItemType File -Force -Path $release
    if ($process -and -not $process.HasExited) {
        $null = $process.WaitForExit(10000)
    }
    if ($process -and -not $process.HasExited) { throw 'identity writer did not exit after release' }
    if ($process -and $process.ExitCode -ne 0) { throw "identity writer exit code $($process.ExitCode)" }
    Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
}
