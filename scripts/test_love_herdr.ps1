# Isolated native LOVE + Herdr lifecycle: preflight, create, runner, controller,
# cancel, cleanup. Runs only against a fresh temporary project and exits normally.
param(
    [string]$Love = 'C:/Program Files/LOVE/lovec.exe'
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$project = Join-Path ([IO.Path]::GetTempPath()) ('uml-70-herdr-' + [guid]::NewGuid().ToString('N'))
$evidence = Join-Path $repo 'acceptance-evidence/70'
$null = New-Item -ItemType Directory -Force -Path (Join-Path $project '.uml-viewer')
$null = New-Item -ItemType Directory -Force -Path $evidence
$fake = Join-Path $project 'claude.exe'
Copy-Item (Join-Path $env:SystemRoot 'System32/ping.exe') $fake
$stdout = Join-Path $evidence 'stdout.txt'
$stderr = Join-Path $evidence 'stderr.txt'
try {
    & $Love (Join-Path $repo 'love') companion-preflight $project --backend herdr 1> $stdout 2> $stderr
    if ($LASTEXITCODE -ne 0) { throw "preflight failed ($LASTEXITCODE): $(Get-Content $stderr -Raw)" }
    $selected = (Get-Content $stdout -Raw).Trim()
    if ($selected -cne 'herdr') { throw "preflight output was not herdr: $selected" }
    Add-Content -LiteralPath (Join-Path $evidence 'preflight.txt') -Value "PASS herdr: $((Get-Content $stderr -Raw).Trim())"
    & $Love (Join-Path $repo 'love') companion-smoke $project $fake 1> $stdout 2> $stderr
    $code = $LASTEXITCODE
    $out = Get-Content $stdout -Raw
    $err = Get-Content $stderr -Raw
    if ($code -ne 0) { throw "smoke failed ($code): $err" }
    if ($out -notmatch 'SMOKE PASS session=(\S+)') { throw "smoke did not report a session: $out $err" }
    $session = $Matches[1]
    $record = Get-Content (Join-Path $project '.uml-viewer/companion.edn') -Raw
    if ($record.Trim() -ne '{}') { throw "ownership was not cleared after cleanup: $record" }
    $inventory = & herdr session list --json | ConvertFrom-Json
    if (@($inventory.sessions | Where-Object { $_.name -eq $session }).Count -ne 0) {
        throw "owned session remains after cleanup: $session"
    }
    Add-Content -LiteralPath (Join-Path $evidence 'lifecycle.txt') -Value "PASS session=$session cleanup=released"
    Copy-Item (Join-Path $project '.uml-viewer/companion.edn') (Join-Path $evidence 'companion.edn.final')
    Copy-Item (Join-Path $project '.uml-viewer/recovery.edn') (Join-Path $evidence 'recovery.edn') -ErrorAction SilentlyContinue
    Copy-Item (Join-Path $project '.uml-viewer/recovery-control.edn') (Join-Path $evidence 'recovery-control.edn') -ErrorAction SilentlyContinue
    $journal = Get-Content (Join-Path $project '.uml-viewer/recovery.edn') -Raw -ErrorAction SilentlyContinue
    Add-Content -LiteralPath (Join-Path $evidence 'lifecycle.txt') -Value "controller journal: $($journal -replace '\s+',' ')"
    $script:passed = $true
    "PASS actual Herdr 0.9.5 companion lifecycle. Session: $session. Evidence: $evidence"
} finally {
    if (-not $script:passed) {
        Add-Content -LiteralPath (Join-Path $evidence 'failure.txt') -Value "kept failed project: $project"
        "FAILED project retained for diagnosis: $project"
    } else {
        Remove-Item -LiteralPath $project -Recurse -Force -ErrorAction SilentlyContinue
    }
}
