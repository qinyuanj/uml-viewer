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
$deadline = [DateTime]::UtcNow.AddSeconds(20)
Add-Type -AssemblyName System.Drawing
$desktopCaptured = $false
while (-not $process.HasExited -and [DateTime]::UtcNow -lt $deadline) {
    $events = Join-Path $EvidenceDirectory 'events.txt'
    $eventText = ''
    if (Test-Path $events) {
        $stream = [IO.File]::Open($events, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::ReadWrite)
        $reader = [IO.StreamReader]::new($stream)
        try { $eventText = $reader.ReadToEnd() } finally { $reader.Dispose() }
    }
    if (-not $desktopCaptured -and $eventText.Contains('native-virtual-keys=NIHAO')) {
        Start-Sleep -Milliseconds 600
        $bitmap = [Drawing.Bitmap]::new(1920,1080)
        $graphics = [Drawing.Graphics]::FromImage($bitmap)
        try {
            $graphics.CopyFromScreen(0,0,0,0,$bitmap.Size)
            $bitmap.Save((Join-Path $EvidenceDirectory 'candidate-desktop.png'))
        } finally { $graphics.Dispose(); $bitmap.Dispose() }
        $desktopCaptured = $true
    }
    Start-Sleep -Milliseconds 100
    $process.Refresh()
}
if (-not $process.HasExited) { throw 'Prototype exceeded deadline; no acceptance pass.' }
if ($process.ExitCode -ne 0) { throw "Prototype exit $($process.ExitCode); inspect stderr.txt" }
$result = Join-Path $EvidenceDirectory 'result.txt'
if (-not (Test-Path $result)) { throw 'Missing prototype acceptance result' }
if (([IO.File]::ReadAllText($result)) -notmatch 'real-ime=pass') { throw 'HARD GATE: real Windows IME composition, candidate confirmation and submission not proven. STOP dependent work.' }
