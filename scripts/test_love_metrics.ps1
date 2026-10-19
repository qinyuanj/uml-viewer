param(
    [string]$Love = 'C:/Program Files/LOVE/lovec.exe',
    [Parameter(Mandatory)][string]$CimguiDirectory
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$evidence = Join-Path ([IO.Path]::GetTempPath()) ('uml-65-metrics-' + [guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory $evidence
$null = New-Item -ItemType Directory "$evidence/.metrics"
$path = Join-Path $evidence 'diagram.edn'
$metric = Join-Path $evidence '.metrics/crap.edn'
[IO.File]::WriteAllText($path, '{:packages [{:label "Adapters" :classes [{:id :ffi :ns "ffi"} {:id :imgui :ns "imgui"}]}] :edges [{:from :ffi :to :imgui}]}')
[IO.File]::WriteAllText($metric, '{:entries [{:namespace "ffi" :name "draw" :complexity 1 :coverage 100 :crap 1}]}')
$original = [IO.File]::GetLastWriteTimeUtc($path)
$env:UML_CIMGUI_DIRECTORY = (Resolve-Path $CimguiDirectory).Path
$env:UML_VIEWER_EVIDENCE = $evidence
$env:UML_VIEWER_TEST_SECONDS = '6'
# Stderr must stay empty: name the font explicitly so no Sarasa fallback warning prints.
$env:UML_CJK_FONT = Join-Path $env:WINDIR 'Fonts/msyh.ttc'
$info = [Diagnostics.ProcessStartInfo]::new($Love)
$info.UseShellExecute = $false
$info.WorkingDirectory = $repo
$info.RedirectStandardOutput = $true
$info.RedirectStandardError = $true
foreach ($a in @((Join-Path $repo 'love'), 'viewer', $path)) { $info.ArgumentList.Add($a) }
$p = [Diagnostics.Process]::Start($info)
$stdout = $p.StandardOutput.ReadToEndAsync()
$stderr = $p.StandardError.ReadToEndAsync()
$clock = [Diagnostics.Stopwatch]::StartNew()
while (-not (Test-Path "$evidence/scene-1.edn") -and -not $p.HasExited -and $clock.Elapsed.TotalSeconds -lt 5) { Start-Sleep -Milliseconds 30 }
if (Test-Path "$evidence/scene-1.edn") {
    # Only the metric changes, with equal byte length; the IR timestamp is untouched.
    [IO.File]::WriteAllText($metric, '{:entries [{:namespace "ffi" :name "draw" :complexity 1 :coverage 100 :crap 9}]}')
    [IO.File]::SetLastWriteTimeUtc($metric, [datetime]::UtcNow.AddSeconds(2))
}
if (-not $p.WaitForExit(15000)) { throw "Isolated viewer failed to exit; own pid $($p.Id); evidence $evidence" }
$out = $stdout.GetAwaiter().GetResult()
$err = $stderr.GetAwaiter().GetResult()
[IO.File]::WriteAllText("$evidence/stdout.txt", $out)
[IO.File]::WriteAllText("$evidence/stderr.txt", $err)
if ($p.ExitCode -ne 0 -or $err) { throw "LOVE exit $($p.ExitCode): $err $out; evidence $evidence" }
foreach ($name in @('scene-1.edn','scene-2.edn','render.png')) { if (-not (Test-Path "$evidence/$name")) { throw "Missing $name; evidence $evidence" } }
$before = [IO.File]::ReadAllText("$evidence/scene-1.edn")
$after = [IO.File]::ReadAllText("$evidence/scene-2.edn")
if ($before -notmatch ':mu 1[ }]' -or $after -notmatch ':mu 9[ }]' -or $before -eq $after) { throw "Metric-only reload failed: $evidence" }
if ([IO.File]::GetLastWriteTimeUtc($path) -ne $original) { throw 'IR timestamp unexpectedly changed' }
if ($after -notmatch ':strokes' -or $after -notmatch ':tip') { throw "Edge renderer scene missing: $evidence" }
Write-Output "PASS: actual metric-only mtime reload, equal-size snapshot, unchanged IR, native edge/class render. Evidence: $evidence"
