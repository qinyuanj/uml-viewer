param(
    [string]$Love = 'C:/Program Files/LOVE/lovec.exe',
    [Parameter(Mandatory)][string]$CimguiDirectory,
    [string]$Font,
    # A wide document renders near 0.8-1.3 fps, so its phases need a longer wall
    # clock than the example's to produce frames at all (~25 frames per phase at
    # that rate). The driver splits whatever budget it is given (2s warmup, then
    # idle/interaction/detail evenly).
    [int]$SyntheticSeconds = 90,
    [int]$MinimumFrames = 15
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
# The driver fails its own run (exit 1) when a phase is starved or the detail
# card never opened, and the runner re-checks every phase's frame count below,
# so a phase that produced no frames can never read as a pass.
try {
    $env:UML_CIMGUI_DIRECTORY = (Resolve-Path $CimguiDirectory).Path
    if ($Font) { $env:UML_CJK_FONT = (Resolve-Path $Font).Path }
    $env:UML_VIEWER_TEST_DRIVER = Join-Path $repo 'scripts/benchmark_viewer.lua'
    $env:UML_VIEWER_PROJECT_ROOT = $evidence
    git rev-parse HEAD | Set-Content "$evidence/revision.txt"
    Get-CimInstance Win32_Processor | Select-Object Name,NumberOfCores,NumberOfLogicalProcessors | ConvertTo-Json | Set-Content "$evidence/cpu.json"
    Get-CimInstance Win32_OperatingSystem | Select-Object Caption,Version,TotalVisibleMemorySize | ConvertTo-Json | Set-Content "$evidence/os.json"
    $cases = @(
        @{Name='example'; Path=(Join-Path $repo 'examples/uml-viewer.edn'); Seconds=16},
        @{Name='synthetic-500'; Path="$evidence/synthetic-500.edn"; Seconds=$SyntheticSeconds}
    )
    $sections = @()
    foreach ($case in $cases) {
        $dir = Join-Path $evidence $case.Name
        $null = New-Item -ItemType Directory $dir
        $env:UML_VIEWER_EVIDENCE = $dir
        $env:UML_VIEWER_TEST_SECONDS = [string]$case.Seconds
        $info = [Diagnostics.ProcessStartInfo]::new($Love)
        $info.UseShellExecute = $false
        $info.WorkingDirectory = $repo
        $info.RedirectStandardOutput = $true
        $info.RedirectStandardError = $true
        foreach ($arg in @((Join-Path $repo 'love'),'viewer',$case.Path)) { $info.ArgumentList.Add($arg) }
        $p = [Diagnostics.Process]::Start($info)
        $stdout = $p.StandardOutput.ReadToEndAsync(); $stderr = $p.StandardError.ReadToEndAsync()
        $limit = ($case.Seconds + 60) * 1000
        if (-not $p.WaitForExit($limit)) { throw "Owned benchmark process did not exit: $($p.Id). Evidence $evidence" }
        $out = $stdout.GetAwaiter().GetResult()
        [IO.File]::WriteAllText("$dir/stdout.txt",$out)
        $err = $stderr.GetAwaiter().GetResult(); [IO.File]::WriteAllText("$dir/stderr.txt",$err)
        # Re-read the published artefacts instead of trusting the console output.
        $record = Test-Path "$dir/benchmark.edn"
        $summaryPath = "$dir/summary.txt"
        if ($p.ExitCode -ne 0 -or -not $record -or -not (Test-Path $summaryPath)) {
            throw "Benchmark failed for $($case.Name) (exit $($p.ExitCode)): $err. Evidence $dir"
        }
        $summaryText = [IO.File]::ReadAllText($summaryPath)
        foreach ($phase in @('idle','interaction','detail')) {
            $match = [regex]::Match($summaryText, "phase=$phase frames=(\d+)")
            if (-not $match.Success) { throw "Benchmark summary for $($case.Name) has no $phase phase. Evidence $dir" }
            if ([int]$match.Groups[1].Value -lt $MinimumFrames) {
                throw "Benchmark $($case.Name) $phase produced $($match.Groups[1].Value) frames (minimum $MinimumFrames). Evidence $dir"
            }
        }
        if ($summaryText -notmatch 'detail opened=true') {
            throw "Benchmark $($case.Name) did not open the detail card. Evidence $dir"
        }
        $sections += "$($case.Name):`n$($summaryText.Trim())"
        Write-Output "case=$($case.Name) PASS"
    }
    [IO.File]::WriteAllText("$evidence/summary.txt", ($sections -join "`n`n") + "`n")
    "Benchmark evidence: $evidence"
} finally {
    foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name,$previous[$name]) }
}
