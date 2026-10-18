# Cold-start comparison: existing JVM IR pipeline vs the LOVE runtime.
# Runs isolated, headless processes on the same fixture policy. It never starts
# a viewer window, a companion, or a recovery controller.
param(
    [string]$Love = 'C:/Program Files/LOVE/lovec.exe',
    [string]$Clj = 'clj',
    [string]$Lua54 = "$env:LOCALAPPDATA/Programs/Lua/bin/lua.exe",
    [ValidateSet('lua', 'typescript', 'rust', 'python')]
    [string]$Language = 'lua',
    [ValidateRange(1, 200)]
    [int]$Runs = 9,
    [ValidateRange(0, 20)]
    [int]$Warmup = 2,
    [string]$MarkdownOut = '',
    [switch]$SelfTest
)
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path

function Get-Median([double[]]$Values) {
    if (-not $Values -or $Values.Count -eq 0) { throw 'median requires at least one sample' }
    $sorted = @($Values | Sort-Object)
    $middle = [int][math]::Floor($sorted.Count / 2)
    if ($sorted.Count % 2 -eq 1) { return $sorted[$middle] }
    return ($sorted[$middle - 1] + $sorted[$middle]) / 2
}

function Get-Summary([double[]]$Values) {
    $sorted = @($Values | Sort-Object)
    [pscustomobject]@{
        count  = $sorted.Count
        median = Get-Median $Values
        mean   = ($Values | Measure-Object -Average).Average
        min    = $sorted[0]
        max    = $sorted[-1]
    }
}

if ($SelfTest) {
    if ((Get-Median @(3.0, 1.0, 2.0)) -ne 2.0) { throw 'odd median failed' }
    if ((Get-Median @(4.0, 1.0, 3.0, 2.0)) -ne 2.5) { throw 'even median failed' }
    if ((Get-Median @(7.5)) -ne 7.5) { throw 'single-sample median failed' }
    try { $null = Get-Median @(); throw 'empty median did not fail' } catch { if ($_.Exception.Message -like '*empty median did not fail*') { throw } }
    'Self-test passed: median math, single sample, and empty input rejection.'
    exit 0
}

function Invoke-Timed {
    param([string]$FileName, [string[]]$Arguments, [string]$WorkingDirectory)
    if (-not $FileName) { throw "empty executable for arguments: $($Arguments -join ' ')" }
    $info = [Diagnostics.ProcessStartInfo]::new($FileName)
    $info.WorkingDirectory = $WorkingDirectory
    $info.UseShellExecute = $false
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    foreach ($argument in $Arguments) { $info.ArgumentList.Add($argument) }
    $watch = [Diagnostics.Stopwatch]::StartNew()
    $process = [Diagnostics.Process]::Start($info)
    $stdout = $process.StandardOutput.ReadToEndAsync()
    $stderr = $process.StandardError.ReadToEndAsync()
    $process.WaitForExit()
    $watch.Stop()
    $out = $stdout.GetAwaiter().GetResult()
    $err = $stderr.GetAwaiter().GetResult()
    [pscustomobject]@{ seconds = $watch.Elapsed.TotalSeconds; exit = $process.ExitCode; stdout = $out; stderr = $err }
}

$fixture = Join-Path $repo "$Language-fixture"
$policy = Join-Path $fixture 'policy.edn'
if (-not (Test-Path -LiteralPath $policy)) { throw "missing fixture policy: $policy" }
if (-not (Test-Path -LiteralPath $Love)) { throw "LOVE executable not found: $Love" }
$cljCommand = Get-Command $Clj -ErrorAction Stop
$sourcePath = (Join-Path $repo 'src').Replace('\', '/')
$extra = '{:paths ["' + $sourcePath + '"] :deps {org.clojure/clojure {:mvn/version "1.12.0"} org.clojure/data.json {:mvn/version "2.5.1"}} :aliases {:ir {:main-opts ["-m" "uml-viewer.main.ir-generator"]}}}'
$work = Join-Path ([IO.Path]::GetTempPath()) ('spec71-benchmark-' + [guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory $work
$jvmOut = Join-Path $work 'jvm.edn'
$loveOut = Join-Path $work 'love.edn'
$jvmSamples = @(); $loveSamples = @()
try {
    if ($Warmup -gt 0) {
        foreach ($index in 1..$Warmup) {
        $jvmWarm = Invoke-Timed -FileName $cljCommand.Source -Arguments @('-Sdeps', $extra, '-M:ir', 'policy.edn', $jvmOut) -WorkingDirectory $fixture
        if ($jvmWarm.exit -ne 0) { throw "JVM warmup $index exited $($jvmWarm.exit): $($jvmWarm.stderr)" }
        $loveWarm = Invoke-Timed -FileName $Love -Arguments @((Join-Path $repo 'love'), 'ir', '--root', $fixture, '--policy', 'policy.edn', '--output', $loveOut) -WorkingDirectory $repo
        if ($loveWarm.exit -ne 0) { throw "LOVE warmup $index exited $($loveWarm.exit): $($loveWarm.stderr)" }
        }
    }
    foreach ($index in 1..$Runs) {
        $result = Invoke-Timed -FileName $cljCommand.Source -Arguments @('-Sdeps', $extra, '-M:ir', 'policy.edn', $jvmOut) -WorkingDirectory $fixture
        if ($result.exit -ne 0) { throw "JVM run $index exited $($result.exit): $($result.stderr)" }
        if (-not (Test-Path $jvmOut) -or (Get-Item $jvmOut).Length -eq 0) { throw "JVM run $index produced no IR" }
        $jvmSamples += $result.seconds
        $loveResult = Invoke-Timed -FileName $Love -Arguments @((Join-Path $repo 'love'), 'ir', '--root', $fixture, '--policy', 'policy.edn', '--output', $loveOut) -WorkingDirectory $repo
        if ($loveResult.exit -ne 0) { throw "LOVE run $index exited $($loveResult.exit): $($loveResult.stderr)" }
        if (-not (Test-Path $loveOut) -or (Get-Item $loveOut).Length -eq 0) { throw "LOVE run $index produced no IR" }
        $loveSamples += $loveResult.seconds
    }
    if ($jvmSamples.Count -ne $Runs -or $loveSamples.Count -ne $Runs) { throw "expected $Runs samples per runtime, got JVM=$($jvmSamples.Count) LOVE=$($loveSamples.Count)" }
    & $Lua54 (Join-Path $repo 'scripts/compare_ir.lua') $loveOut $jvmOut | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'LOVE and JVM produced semantically different IR for the benchmark fixture' }
    $jvmSummary = Get-Summary $jvmSamples
    $loveSummary = Get-Summary $loveSamples
    $speedup = $jvmSummary.median / $loveSummary.median
    $markdown = @(
        '# Spec #50 cold-start benchmark',
        '',
        "- Date: $([datetime]::UtcNow.ToString('yyyy-MM-ddTHH:mm:ssZ'))",
        "- Fixture: ``$Language-fixture`` (policy.edn), same input for both runtimes",
        "- Runs: $Runs measured after $Warmup warmup runs per runtime",
        '- Measurement: fresh OS process from start to exit, headless IR generation (`clj -M:ir` vs `love <app> ir`); no viewer window, companion, or recovery controller.',
        '- Both outputs compared semantically with `scripts/compare_ir.lua` after the runs.',
        '',
        '| Runtime | Median (s) | Mean (s) | Min (s) | Max (s) |',
        '| --- | ---: | ---: | ---: | ---: |',
        "| JVM (clj -M:ir) | $('{0:F3}' -f $jvmSummary.median) | $('{0:F3}' -f $jvmSummary.mean) | $('{0:F3}' -f $jvmSummary.min) | $('{0:F3}' -f $jvmSummary.max) |",
        "| LOVE (love ir) | $('{0:F3}' -f $loveSummary.median) | $('{0:F3}' -f $loveSummary.mean) | $('{0:F3}' -f $loveSummary.min) | $('{0:F3}' -f $loveSummary.max) |",
        '',
        "Median speedup (JVM / LOVE): $('{0:F2}' -f $speedup)x",
        '',
        'Raw samples (seconds):',
        '',
        '```',
        "JVM:  $($jvmSamples -join ', ')",
        "LOVE: $($loveSamples -join ', ')",
        '```',
        ''
    ) -join "`n"
    if (-not $MarkdownOut) { $MarkdownOut = Join-Path ([IO.Path]::GetTempPath()) ('spec71-cold-start-benchmark-' + (git rev-parse --short HEAD) + '.md') }
    [IO.File]::WriteAllText($MarkdownOut, $markdown, [Text.UTF8Encoding]::new($false))
    $markdown
    "Benchmark artifacts: $work"
    "Markdown: $MarkdownOut"
} finally {
    if ($jvmSamples.Count -eq 0 -and $loveSamples.Count -eq 0) { Remove-Item -Recurse -Force $work -Confirm:$false -ErrorAction SilentlyContinue }
}
