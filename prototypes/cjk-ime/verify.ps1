param(
    [Parameter(Mandatory)][string]$DependencyDirectory,
    [string]$Love = 'C:\Program Files\LOVE\lovec.exe',
    [string]$EvidenceDirectory = (Join-Path $PSScriptRoot 'evidence')
)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force $EvidenceDirectory | Out-Null
foreach ($name in @('result.txt', 'failure.txt', 'events.txt', 'cjk-render.png', 'ime-before-confirmation.png', 'ime-after-submission.png')) {
    $old = Join-Path $EvidenceDirectory $name
    if (Test-Path $old) { Remove-Item $old -Confirm:$false }
}
$env:UML52_DEPENDENCIES = (Resolve-Path $DependencyDirectory).Path
$env:UML52_EVIDENCE = (Resolve-Path $EvidenceDirectory).Path
$process = Start-Process $Love -ArgumentList @('"' + $PSScriptRoot + '"') -PassThru -RedirectStandardOutput (Join-Path $EvidenceDirectory 'stdout.txt') -RedirectStandardError (Join-Path $EvidenceDirectory 'stderr.txt')
if (-not $process.WaitForExit(20000)) { throw 'Prototype exceeded deadline; no acceptance pass.' }
if ($process.ExitCode -ne 0) { throw "Prototype exit $($process.ExitCode); inspect stderr.txt" }
$result = Join-Path $EvidenceDirectory 'result.txt'
if (-not (Test-Path $result)) { throw 'Missing prototype acceptance result' }
if (([IO.File]::ReadAllText($result)) -notmatch 'real-ime=pass') { throw 'HARD GATE: real Windows IME composition, candidate confirmation and submission not proven. STOP dependent work.' }
