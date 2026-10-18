# Native Windows wrapper check: LOVE identity recognition and cross-runtime refusal.
param()
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$root = Join-Path ([IO.Path]::GetTempPath()) ('uml-69-wrapper-' + [guid]::NewGuid().ToString('N'))
$installed = Join-Path $root '.uml-viewer/uml-viewer'
$null = New-Item -ItemType Directory -Force -Path (Join-Path $installed 'love')
Copy-Item -Recurse -LiteralPath (Join-Path $repo 'scripts') -Destination $installed
$directory = Join-Path $root '.uml-viewer'
$launcher = Join-Path $directory 'viewer-process.json'
$identity = Join-Path $directory 'viewer-process-identity.json'
$record = Join-Path $root 'love-args.json'
$stub = Join-Path $root 'love stub.ps1'
$previousLove, $previousRecord = $env:UML_LOVE_EXECUTABLE, $env:UML69_RECORD
try {
    Set-Content -LiteralPath $stub -Value @'
if ($args -contains 'companion-preflight') {
    & cmd.exe /d /s /c 'echo uml: selected Herdr 0.9.5 at fixture 1>&2'
    Write-Output 'herdr'
    exit 0
}
$args | ConvertTo-Json -AsArray | Set-Content -LiteralPath $env:UML69_RECORD
'@ -Encoding utf8
    $env:UML_LOVE_EXECUTABLE = $stub
    $env:UML69_RECORD = $record
    $scripts = Join-Path $installed 'scripts/uml-command.ps1'

    # A recorded dead LOVE process is positively absent, so the wrapper accepts
    # the identity and relaunches with the same runtime.
    Set-Content -LiteralPath $launcher -Value '{"pid": 2147483647, "started": "1", "runtime": "love"}' -Encoding utf8
    Set-Content -LiteralPath $identity -Value '{"pid": 2147483647, "started": "4294967296", "runtime": "love"}' -Encoding utf8
    $output = (& pwsh -NoProfile -File $scripts $root '--runtime' 'love' '--restart' 'diagram.edn' 2>&1 | Out-String)
    $code = $LASTEXITCODE
    if ($code -ne 0) { throw "LOVE restart failed ($code): $output" }
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    while (-not (Test-Path -LiteralPath $record) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 50 }
    if (-not (Test-Path -LiteralPath $record)) { throw 'LOVE restart did not invoke the runtime fixture' }
    $argv = Get-Content -LiteralPath $record -Raw | ConvertFrom-Json
    if (($argv[-5..-1] -join ' ') -cne 'viewer --backend herdr --restart diagram.edn') { throw "unexpected viewer argv: $($argv -join ' ')" }
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    while (@(Get-ChildItem -LiteralPath $directory -Filter 'launch-*.json').Count -ne 0 -and
           [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 50 }

    # A JVM-owned session must refuse LOVE restart before any ownership write.
    $before = '{"pid": 2147483647, "started": "1", "runtime": "jvm"}'
    Set-Content -LiteralPath $launcher -Value $before -Encoding utf8 -NoNewline
    Remove-Item -LiteralPath $record -Force
    $output = (& pwsh -NoProfile -File $scripts $root '--runtime' 'love' '--restart' 2>&1 | Out-String)
    $code = $LASTEXITCODE
    if ($code -eq 0) { throw 'cross-runtime restart must fail' }
    if ($output -notmatch 'Cannot restart a JVM viewer as love') { throw "unclear refusal: $output" }
    if ((Get-Content -LiteralPath $launcher -Raw) -cne $before) { throw 'refused restart changed launcher ownership' }
    if (Test-Path -LiteralPath $record) { throw 'refused restart launched the LOVE runtime' }
    if (@(Get-ChildItem -LiteralPath $directory -Filter 'launch-*.json').Count -ne 0) { throw 'refused restart wrote a launch manifest' }
    Write-Output 'LOVE restart identity recognition and cross-runtime refusal verified'
} finally {
    $env:UML_LOVE_EXECUTABLE, $env:UML69_RECORD = $previousLove, $previousRecord
    Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
}
