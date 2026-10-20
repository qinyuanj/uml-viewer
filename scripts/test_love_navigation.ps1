param(
    [string]$Love = 'C:/Program Files/LOVE/lovec.exe',
    [Parameter(Mandatory)][string]$CimguiDirectory
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$evidence = Join-Path ([IO.Path]::GetTempPath()) ('uml-64-navigation-' + [guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory $evidence
$path = Join-Path $evidence 'diagram.edn'
$children = (1..20 | ForEach-Object { '{:id :source.child' + $_ + '}' }) -join ' '
$ops = (1..70 | ForEach-Object { '"operation' + $_ + '"' }) -join ' '
[IO.File]::WriteAllText($path, ('{:hierarchical true :title "Navigation acceptance" :order [:source :other] :classes [{:id :source} {:id :source.lua} {:id :source.lua.parser} ' + $children + ' {:id :other :ops [' + $ops + ']}] :edges []}'))
$names = @('UML_CIMGUI_DIRECTORY','UML_VIEWER_EVIDENCE','UML_VIEWER_TEST_SECONDS','UML_VIEWER_TEST_DRIVER')
$previous = @{}
foreach ($name in $names) { $previous[$name] = [Environment]::GetEnvironmentVariable($name) }
try {
    $env:UML_CIMGUI_DIRECTORY = (Resolve-Path $CimguiDirectory).Path
    $env:UML_VIEWER_EVIDENCE = $evidence
    $env:UML_VIEWER_TEST_SECONDS = '8'
    $env:UML_VIEWER_TEST_DRIVER = Join-Path $repo 'scripts/test_love_navigation.lua'
    $info = [Diagnostics.ProcessStartInfo]::new($Love)
    $info.UseShellExecute = $false
    $info.WorkingDirectory = $repo
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    foreach ($a in @((Join-Path $repo 'love'), 'viewer', $path)) { $info.ArgumentList.Add($a) }
    $p = [Diagnostics.Process]::Start($info)
    $stdout = $p.StandardOutput.ReadToEndAsync()
    $stderr = $p.StandardError.ReadToEndAsync()
    if (-not $p.WaitForExit(20000)) { throw "Owned navigation process did not exit: pid $($p.Id), evidence $evidence" }
    $out = $stdout.GetAwaiter().GetResult()
    $err = $stderr.GetAwaiter().GetResult()
    [IO.File]::WriteAllText("$evidence/stdout.txt", $out)
    [IO.File]::WriteAllText("$evidence/stderr.txt", $err)
    if ($p.ExitCode -ne 0) { throw "Navigation failed ($($p.ExitCode)): $err $out Evidence: $evidence" }
    foreach ($file in @('navigation-pass.txt','navigation-entered.edn','navigation-zoomed.edn','navigation-panned.edn','navigation-fitted.edn','navigation-back.edn','navigation-selected.edn','navigation-wheel.edn','render.png')) {
        if (-not (Test-Path "$evidence/$file")) { throw "Missing $file. $err $out Evidence: $evidence" }
    }
    "PASS actual LOVE/cimgui navigation callbacks and rendered camera. Evidence: $evidence"
} finally {
    foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name, $previous[$name]) }
}
