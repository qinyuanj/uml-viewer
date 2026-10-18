param(
    [string]$Love = 'C:/Program Files/LOVE/lovec.exe',
    [Parameter(Mandatory)][string]$CimguiDirectory
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$evidence = Join-Path ([IO.Path]::GetTempPath()) ('uml-62-render-' + [guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory $evidence
$path = Join-Path $evidence 'diagram.edn'
Copy-Item (Join-Path $repo 'examples/uml-viewer.edn') $path
$env:UML_CIMGUI_DIRECTORY = (Resolve-Path $CimguiDirectory).Path
$env:UML_VIEWER_EVIDENCE = $evidence
$env:UML_VIEWER_TEST_SECONDS = '8'
$info = [Diagnostics.ProcessStartInfo]::new($Love)
$info.UseShellExecute = $false
$info.WorkingDirectory = $repo
$info.RedirectStandardOutput = $true
$info.RedirectStandardError = $true
foreach ($a in @((Join-Path $repo 'love'), 'viewer', $path)) { $info.ArgumentList.Add($a) }
$p = [Diagnostics.Process]::Start($info)
$stdout = $p.StandardOutput.ReadToEndAsync()
$stderr = $p.StandardError.ReadToEndAsync()
$deadline = [datetime]::UtcNow.AddSeconds(6)
while (-not (Test-Path "$evidence/scene-1.edn") -and -not $p.HasExited -and [datetime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 50 }
if (Test-Path "$evidence/scene-1.edn") {
    # Real file content + mtime change, independent of viewer callbacks.
    $text = [IO.File]::ReadAllText($path)
    $text = $text.Replace('UML viewer', 'Reloaded UML viewer')
    [IO.File]::WriteAllText($path, $text + "`n; isolated reload trigger`n")
    [IO.File]::SetLastWriteTimeUtc($path, [datetime]::UtcNow.AddSeconds(2))
}
if (-not $p.WaitForExit(20000)) { throw "Isolated renderer did not exit; own pid $($p.Id), evidence $evidence" }
$out = $stdout.GetAwaiter().GetResult()
$err = $stderr.GetAwaiter().GetResult()
[IO.File]::WriteAllText("$evidence/stdout.txt", $out)
[IO.File]::WriteAllText("$evidence/stderr.txt", $err)
if ($p.ExitCode -ne 0) { throw "LOVE failed ($($p.ExitCode)): $err $out Evidence: $evidence" }
foreach ($file in @('scene-1.edn', 'scene-2.edn', 'render.png')) {
    if (-not (Test-Path "$evidence/$file")) { throw "Missing $file. $err $out Evidence: $evidence" }
}
Write-Output "PASS: actual cimgui render of repository IR and automatic mtime reload. Evidence: $evidence"
