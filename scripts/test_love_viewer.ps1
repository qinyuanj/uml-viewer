param(
    [string]$Love = 'C:/Program Files/LOVE/lovec.exe',
    [Parameter(Mandatory)][string]$CimguiDirectory
)
$ErrorActionPreference = 'Stop'
function Wait-UmlViewerScene($Path,$Process,[int]$TimeoutSeconds) {
    $deadline=[datetime]::UtcNow.AddSeconds($TimeoutSeconds)
    while (-not (Test-Path -LiteralPath $Path) -and -not $Process.HasExited -and [datetime]::UtcNow -lt $deadline) {
        Start-Sleep -Milliseconds 50
    }
    if (-not (Test-Path -LiteralPath $Path)) { throw "Viewer did not publish $Path within $TimeoutSeconds seconds (exited=$($Process.HasExited))." }
}
function Invoke-UmlViewerReload($Path,$Evidence,$Process,[scriptblock]$Wait = ${function:Wait-UmlViewerScene}) {
    & $Wait "$Evidence/scene-1.edn" $Process 15
    $first=[datetime]::UtcNow
    $text=[IO.File]::ReadAllText($Path)
    if (-not $text.Contains('UML viewer')) { throw 'Reload fixture lacks the expected title.' }
    [IO.File]::WriteAllText($Path,$text.Replace('UML viewer','Reloaded UML viewer') + "`n; isolated reload trigger`n")
    [IO.File]::SetLastWriteTimeUtc($Path,[datetime]::UtcNow.AddSeconds(2))
    & $Wait "$Evidence/scene-2.edn" $Process 15
    "scene-1 observed=$($first.ToString('o')); scene-2 observed=$([datetime]::UtcNow.ToString('o'))"
}
$repo = Split-Path $PSScriptRoot -Parent
$evidence = Join-Path ([IO.Path]::GetTempPath()) ('uml-62-render-' + [guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory $evidence
$path = Join-Path $evidence 'diagram.edn'
Copy-Item (Join-Path $repo 'examples/uml-viewer.edn') $path
$env:UML_CIMGUI_DIRECTORY = (Resolve-Path $CimguiDirectory).Path
$env:UML_VIEWER_EVIDENCE = $evidence
$env:UML_VIEWER_TEST_SECONDS = '20'
$env:UML_VIEWER_PROJECT_ROOT = $evidence
$info = [Diagnostics.ProcessStartInfo]::new($Love)
$info.UseShellExecute = $false
$info.WorkingDirectory = $evidence
$info.RedirectStandardOutput = $true
$info.RedirectStandardError = $true
foreach ($a in @((Join-Path $repo 'love'), 'viewer', $path)) { $info.ArgumentList.Add($a) }
$p = [Diagnostics.Process]::Start($info)
$stdout = $p.StandardOutput.ReadToEndAsync()
$stderr = $p.StandardError.ReadToEndAsync()
$reloadError=$null
try { Invoke-UmlViewerReload $path $evidence $p }
catch { $reloadError=$_ }
if (-not $p.WaitForExit(30000)) { throw "Isolated renderer did not exit; own pid $($p.Id), evidence $evidence" }
$out = $stdout.GetAwaiter().GetResult()
$err = $stderr.GetAwaiter().GetResult()
[IO.File]::WriteAllText("$evidence/stdout.txt", $out)
[IO.File]::WriteAllText("$evidence/stderr.txt", $err)
if ($reloadError) { throw "$reloadError Evidence: $evidence" }
if ($p.ExitCode -ne 0) { throw "LOVE failed ($($p.ExitCode)): $err $out Evidence: $evidence" }
foreach ($file in @('scene-1.edn', 'scene-2.edn', 'render.png')) {
    if (-not (Test-Path "$evidence/$file")) { throw "Missing $file. $err $out Evidence: $evidence" }
}
$before = [IO.File]::ReadAllText("$evidence/scene-1.edn")
$after = [IO.File]::ReadAllText("$evidence/scene-2.edn")
if ($before -eq $after -or -not $after.Contains('Reloaded UML viewer')) { throw "mtime reload did not publish changed scene content: $evidence" }
Write-Output "PASS: actual cimgui render of repository IR and automatic mtime reload. Evidence: $evidence"
