param(
    [string]$Love = 'C:/Program Files/LOVE/lovec.exe',
    [Parameter(Mandatory)][string]$CimguiDirectory,
    [string]$Font
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$evidence = Join-Path ([IO.Path]::GetTempPath()) ('uml-87-benchmark-' + [guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory $evidence
# An independent wide namespace exercises 500 visible class boxes and 499 edges.
$classes = (1..500 | ForEach-Object { '{:id :class' + $_ + ' :name "Class' + $_ + '" :ns "synthetic.class' + $_ + '"}' }) -join ' '
$edges = (1..499 | ForEach-Object { '{:from :class' + $_ + ' :to :class' + ($_ + 1) + ' :kind :dependency}' }) -join ' '
[IO.File]::WriteAllText("$evidence/synthetic-500.edn", ('{:hierarchical true :title "Synthetic 500" :classes [' + $classes + '] :edges [' + $edges + ']}'))
$names = @('UML_CIMGUI_DIRECTORY','UML_CJK_FONT','UML_VIEWER_EVIDENCE','UML_VIEWER_TEST_SECONDS','UML_VIEWER_TEST_DRIVER','UML_VIEWER_PROJECT_ROOT')
$previous = @{}
foreach ($name in $names) { $previous[$name] = [Environment]::GetEnvironmentVariable($name) }
try {
    $env:UML_CIMGUI_DIRECTORY = (Resolve-Path $CimguiDirectory).Path
    if ($Font) { $env:UML_CJK_FONT = (Resolve-Path $Font).Path }
    $env:UML_VIEWER_TEST_SECONDS = '16'
    $env:UML_VIEWER_TEST_DRIVER = Join-Path $repo 'scripts/benchmark_viewer.lua'
    $env:UML_VIEWER_PROJECT_ROOT = $evidence
    Get-CimInstance Win32_Processor | Select-Object Name,NumberOfCores,NumberOfLogicalProcessors | ConvertTo-Json | Set-Content "$evidence/cpu.json"
    Get-CimInstance Win32_OperatingSystem | Select-Object Caption,Version,TotalVisibleMemorySize | ConvertTo-Json | Set-Content "$evidence/os.json"
    foreach ($case in @(@{Name='example';Path=(Join-Path $repo 'examples/uml-viewer.edn')},@{Name='synthetic-500';Path="$evidence/synthetic-500.edn"})) {
        $dir = Join-Path $evidence $case.Name
        $null = New-Item -ItemType Directory $dir
        $env:UML_VIEWER_EVIDENCE = $dir
        $info = [Diagnostics.ProcessStartInfo]::new($Love)
        $info.UseShellExecute = $false
        $info.WorkingDirectory = $repo
        $info.RedirectStandardOutput = $true
        $info.RedirectStandardError = $true
        foreach ($arg in @((Join-Path $repo 'love'),'viewer',$case.Path)) { $info.ArgumentList.Add($arg) }
        $p = [Diagnostics.Process]::Start($info)
        $stdout = $p.StandardOutput.ReadToEndAsync(); $stderr = $p.StandardError.ReadToEndAsync()
        if (-not $p.WaitForExit(30000)) { throw "Owned benchmark process did not exit: $($p.Id). Evidence $evidence" }
        [IO.File]::WriteAllText("$dir/stdout.txt",$stdout.GetAwaiter().GetResult())
        $err = $stderr.GetAwaiter().GetResult(); [IO.File]::WriteAllText("$dir/stderr.txt",$err)
        if ($p.ExitCode -ne 0 -or -not (Test-Path "$dir/benchmark.edn")) { throw "Benchmark failed ($($p.ExitCode)): $err. Evidence $dir" }
    }
    "Benchmark evidence: $evidence"
} finally {
    foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name,$previous[$name]) }
}
