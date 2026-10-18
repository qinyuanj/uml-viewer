# Shared installed command runtime. Root is supplied by the thin project entry.
if ($PSVersionTable.PSVersion.Major -lt 7) {
    Write-Error 'PowerShell 7 is required: https://aka.ms/powershell-install'; exit 1
}
$ErrorActionPreference = 'Stop'
$root = $args[0]
$commandArgs = @($args | Select-Object -Skip 1)
function Clojure-Command {
    $command = Get-Command clojure -ErrorAction SilentlyContinue
    if ($IsWindows -and (-not $command -or $command.CommandType -ne 'Application')) {
        # Importing the official module makes its clojure alias shadow any
        # PATH .ps1 shim (alias precedence), so split-form detection below
        # sees the real Invoke-Clojure entry, not the forwarding script.
        Import-Module ClojureTools -ErrorAction SilentlyContinue
        $command = Get-Command clojure -ErrorAction SilentlyContinue
    }
    if (-not $command) { throw 'Clojure CLI is required. Install: https://clojure.org/guides/install_clojure' }
    if ($IsWindows -and $command.CommandType -eq 'Application' -and
        [IO.Path]::GetExtension($command.Source).ToLowerInvariant() -in @('.cmd', '.bat')) {
        throw 'Clojure .cmd/.bat wrappers cannot safely launch the background viewer. Put clojure.exe from the Windows Clojure CLI installer on PATH (https://clojure.org/guides/install_clojure), or use the ClojureTools exported clojure command / a .ps1 entry. Remove the batch wrapper shadowing that entry.'
    }
    return $command
}
try {
    Set-Location -LiteralPath $root
    $toolRoot = (Resolve-Path -LiteralPath '.uml-viewer/uml-viewer').Path.Replace('\', '/')
    $escapedRoot = $toolRoot.Replace('"', '\"')
    $deps = '{:deps {uml-viewer/uml-viewer {:local/root "' + $escapedRoot + '"} quil/quil {:mvn/version "4.3.1563"}}}'
    $runtime = 'jvm'
    $remaining = @()
    $runtimeSeen = $false
    for ($i = 0; $i -lt $commandArgs.Count; $i++) {
        if ($commandArgs[$i] -ceq '--runtime') {
            if ($runtimeSeen -or $i + 1 -ge $commandArgs.Count -or $commandArgs[$i + 1] -cnotin @('love', 'jvm')) { throw '--runtime requires exactly one love or jvm value.' }
            $runtimeSeen = $true
            $runtime = $commandArgs[++$i]
        } else { $remaining += $commandArgs[$i] }
    }
    $commandArgs = $remaining
    if ($commandArgs.Count -gt 0 -and $commandArgs[0] -in @('-h', '--help')) {
        Write-Output 'usage: uml [--runtime jvm|love] [--backend herdr] [--restart | ir | crap | mutate] [args]'; exit 0
    }
    if ($runtime -eq 'love') {
        . "$PSScriptRoot/uml-love.ps1"
        Invoke-UmlLove $root $toolRoot $commandArgs
        exit $LASTEXITCODE
    }
    if ($commandArgs.Count -gt 0 -and $commandArgs[0] -eq 'ir') {
        $name = Split-Path $root -Leaf
        $policy = if (Test-Path -LiteralPath "examples/$name.policy.edn") {
            "examples/$name.policy.edn"
        } else {
            $first = Get-ChildItem 'examples/*.policy.edn' -ErrorAction SilentlyContinue | Sort-Object Name | Select-Object -First 1
            if ($first) { $first.FullName } elseif (Test-Path 'policy.edn') { 'policy.edn' }
        }
        if (-not $policy) { throw "no policy. Write examples/$name.policy.edn." }
        $cli = Clojure-Command
        $forward = @($commandArgs | Select-Object -Skip 1)
        & $cli -Sdeps $deps -M -m uml-viewer.main.ir-generator $policy @forward
        exit $LASTEXITCODE
    }
    if ($commandArgs.Count -gt 0 -and $commandArgs[0] -in @('crap', 'mutate')) {
        $alias = $commandArgs[0]
        $forward = @($commandArgs | Select-Object -Skip 1)
        . "$PSScriptRoot/uml-quality-config.ps1"
        $resultPath = Join-Path $root '.uml-viewer/quality-result.json'
        if (Test-Path -LiteralPath $resultPath) { Remove-Item -LiteralPath $resultPath -Force }
        $qualityConfig = Read-UmlQualityConfig $root
        if ($null -ne $qualityConfig -and $qualityConfig.ContainsKey('operations')) {
            $result = Invoke-UmlConfiguredQuality $root $alias $forward $qualityConfig
            Write-UmlQualityResult $root $alias $result
            exit $result.exitCode
        }
        $standard = if ($null -ne $qualityConfig) { $qualityConfig.standardTools } else { $null }
        if ((Test-Path 'deps.edn') -and $null -eq $standard) {
            $cli = Clojure-Command
            # Official ClojureTools treats '-M:<alias>' as a source file; its
            # protocol requires the literal '-M:' plus the alias.
            if ($cli.ModuleName -eq 'ClojureTools' -or $cli.Definition -eq 'Invoke-Clojure') {
                & $cli '-M:' $alias @forward
            } else {
                & $cli "-M:$alias" @forward
            }
            $code = $LASTEXITCODE
            # Generic aliases accept only zero; repositories may declare their pinned tool contract.
            $codes = @(0)
            if ($null -ne $qualityConfig -and $qualityConfig.ContainsKey('aliasResultExitCodes') -and
                $qualityConfig.aliasResultExitCodes.ContainsKey($alias)) {
                $codes = @($qualityConfig.aliasResultExitCodes[$alias])
            }
            Write-UmlQualityResult $root $alias @{
                exitCode = $code; validResult = ($codes -contains $code);
                crapScope = $(if ($alias -eq 'crap') { 'targets' } else { $null });
                configured = $false; tool = "clojure-alias:$alias"
            }
            exit $code
        }
        if ($null -ne $standard) {
            . "$PSScriptRoot/uml-standard-quality.ps1"
            if (@($forward | Where-Object { $_ -in @('--root', '--source-root', '-s', '--exclude') }).Count -gt 0) {
                throw 'Standard scope is configured in standardTools; refresh arguments cannot override roots or exclusions.'
            }
            $null = @(Get-UmlBusinessRoots $root $standard)
            if ($alias -eq 'mutate') { $forward = @(Select-UmlMutationTargets $root $standard $forward) }
            $scopeArgs = @()
            foreach ($sourceRoot in $standard.sourceRoots) { $scopeArgs += @('--source-root', $sourceRoot) }
            if ($alias -eq 'crap') {
                foreach ($exclusion in $standard.exclude) { $scopeArgs += @('--exclude', $exclusion) }
                $forward = $scopeArgs + @($standard.crapperOptions) + $forward
            } else {
                # Mutator prioritizes --source-root over explicit files; the
                # adapter already applied roots, so pass only filtered targets.
                $forward = @($standard.mutatorOptions) + $forward
            }
        }
        # crapper replaces its snapshot, so standard refresh must select the
        # complete business tree. Its parser rejects partial paths/--changed
        # instead of silently dropping metrics belonging to other classes.
        if ($alias -eq 'crap' -and $forward -notcontains '--full-tree') {
            $forward = @('--full-tree') + $forward
        }
        $tool = if ($alias -eq 'crap') { 'crapper' } else { 'mutator' }
        $directory = Join-Path $root ".uml-viewer/$tool"
        if ($IsWindows) {
            $python = Join-Path $directory '.venv/Scripts/python.exe'
            if (-not (Test-Path -LiteralPath $python)) {
                throw "$python is missing. Install Python 3.11+ (https://www.python.org/downloads/), re-run get-uml-viewer if the checkout is missing, then run: py -3 -m venv `"$directory/.venv`"; & `"$python`" -m pip install -e `"$directory`". The installer does not install dependencies."
            }
            & $python -m $tool @forward
        } else {
            $entry = Join-Path $directory $tool
            if (-not (Test-Path -LiteralPath $entry)) {
                throw "$entry is missing. Re-run get-uml-viewer (set $($tool.ToUpper())_REPO_URL if the clone failed). Install Python 3.11+: https://www.python.org/downloads/"
            }
            & $entry @forward
        }
        $code = $LASTEXITCODE
        Write-UmlQualityResult $root $alias @{
            exitCode = $code; validResult = $(if ($alias -eq 'crap') { $code -in @(0, 2) } else { $code -in @(0, 3) });
            crapScope = $(if ($alias -eq 'crap') { 'whole-tree' } else { $null });
            configured = $false; tool = $tool
        }
        exit $code
    }
    if ($commandArgs -ccontains '--restart') {
        # Refuse a cross-runtime restart before requiring Clojure or running the
        # read-only backend preflight. This keeps the refusal independent of the
        # JVM toolchain that a LOVE session may no longer have.
        . "$PSScriptRoot/uml-process-identity.ps1"
        $earlyRecordPath = Join-Path $root '.uml-viewer/viewer-process.json'
        if (Test-Path -LiteralPath $earlyRecordPath) {
            $earlyRecord = Get-Content -LiteralPath $earlyRecordPath -Raw | ConvertFrom-Json
            $null = Assert-UmlRestartRuntime $root $earlyRecord 'jvm'
        }
    }
    $cli = Clojure-Command
    # One lightweight JVM implements selection and EDN ownership parsing for
    # both platforms. It loads no Quil and creates no launch manifest or owner.
    $platform = if ($IsWindows) { 'Windows' } elseif ($IsMacOS) { 'Mac OS X' } else { 'Linux' }
    $checked = @(& $cli -Sdeps $deps -M -m uml-viewer.adapters.companion $root $platform @commandArgs)
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    if ($checked.Count -ne 1 -or $checked[0] -cne 'herdr') {
        throw 'Backend preflight returned invalid output; no viewer was launched.'
    }
    $selectedBackend = [string]$checked[0]
    $forwarded = @()
    for ($i = 0; $i -lt $commandArgs.Count; $i++) {
        if ($commandArgs[$i] -ceq '--backend') { $i++ } else { $forwarded += $commandArgs[$i] }
    }
    $commandArgs = $forwarded
    if ($commandArgs -ccontains '--restart') {
        # The launcher and runtime each persist identity. Restart validates the
        # recorded runtime first, then waits for both exact recorded processes
        # before starting their replacements.
        $owner = Get-UmlLauncherOwner $root
        $null = Assert-UmlRestartRuntime $root $owner.Record 'jvm'
        Wait-UmlLauncherOwner $owner 'JVM'
        $jvmRecord = Join-Path $root '.uml-viewer/viewer-jvm.json'
        if (-not (Test-Path -LiteralPath $jvmRecord)) {
            throw 'No viewer JVM process record; cannot safely restart without authoritative JVM identity.'
        }
        $jvmJson = Get-Content -LiteralPath $jvmRecord -Raw
        $jvm = $jvmJson | ConvertFrom-Json
        if (-not $jvm.pid -or -not $jvm.started) { throw 'Invalid viewer JVM process record; cannot safely restart.' }
        # PS 7.5 ConvertFrom-Json converts ISO strings to DateTime automatically;
        # retain the writer's fractional precision from the original JSON string.
        $document = [System.Text.Json.JsonDocument]::Parse($jvmJson)
        try { $started = $document.RootElement.GetProperty('started').GetString() }
        finally { $document.Dispose() }
        # Java Instant.toString uses UTC and 0/3/6/9 fractional digits; also
        # accept the existing .NET seven-digit round-trip records exactly.
        # Windows ProcessHandle converts FILETIME to whole milliseconds (observed
        # .900Z vs .NET .9008143Z on JDK 21). Compare those records to the
        # truncated millisecond, not a tolerance around the recorded instant.
        # Finer legacy records must match exactly; .NET cannot confirm sub-ticks.
        $instant = [regex]::Match($started, '^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2})(?:\.(\d{3}|\d{6}|\d{7}|\d{9}))?Z$')
        $recordedStart = [datetime]::MinValue
        $fraction = $instant.Groups[2].Value
        if (-not $instant.Success -or
            ($fraction.Length -eq 9 -and $fraction.Substring(7) -ne '00') -or
            -not [datetime]::TryParseExact(
                ($instant.Groups[1].Value + '.' + $fraction.PadRight(7, '0').Substring(0, 7) + 'Z'),
                "yyyy-MM-dd'T'HH:mm:ss.fffffff'Z'", [Globalization.CultureInfo]::InvariantCulture,
                [Globalization.DateTimeStyles]::AssumeUniversal -bor [Globalization.DateTimeStyles]::AdjustToUniversal,
                [ref]$recordedStart)) {
            throw 'Invalid viewer JVM start time; cannot safely restart.'
        }
        $oldJvm = Get-Process -Id $jvm.pid -ErrorAction SilentlyContinue
        if ($oldJvm) {
            $actualTicks = $oldJvm.StartTime.ToUniversalTime().Ticks
            if ($IsWindows -and $fraction.Length -le 3) { $actualTicks -= $actualTicks % [TimeSpan]::TicksPerMillisecond }
            if ($oldJvm.Name -notmatch '^(java|javaw|clojure)$' -or
                $actualTicks -ne $recordedStart.Ticks) {
                throw 'Viewer JVM process identity changed; cannot safely restart.'
            }
            Write-Output 'Waiting for the old viewer JVM after :quit-for-restart...'
            if (-not $oldJvm.WaitForExit(30000)) {
                throw 'Old viewer JVM is still running. Send :quit-for-restart, wait for its exit, then retry ./uml --restart.'
            }
        }
        # Preserve explicit path/argv; an omitted restart path stays omitted so the
        # JVM restores the diagram path saved by :quit-for-restart instead of
        # restarting a guessed project example.
    }
    $commandArgs = @('--backend', $selectedBackend) + $commandArgs
    # 参数写入数据文件；后台 PS7 进程直接用 ArgumentList 启动 JVM，不经 shell 再解析。
    $directory = Join-Path $root '.uml-viewer'
    $null = New-Item -ItemType Directory -Force -Path $directory
    $manifest = Join-Path $directory ('launch-' + [guid]::NewGuid().ToString('N') + '.json')
    $log = Join-Path $root 'uml-viewer-log.txt'
    $psCli = $cli.CommandType -ne 'Application'
    if ($psCli -and $cli.CommandType -ne 'ExternalScript' -and -not $cli.ModuleName) {
        throw 'Background Clojure CLI must be an executable, a .ps1 entry, or an exported module command.'
    }
    @{ executable = $(if ($cli.CommandType -eq 'ExternalScript' -or -not $psCli) { $cli.Source } else { $cli.Name });
       powershell = $psCli; module = $cli.ModuleName; cwd = $root; log = $log; runtime = 'jvm';
       argv = @('-Sdeps', $deps, '-M', '-m', 'uml-viewer.main.uml-viewer') + $commandArgs } |
        ConvertTo-Json -Depth 4 -Compress | Set-Content -LiteralPath $manifest -Encoding utf8
    $info = [Diagnostics.ProcessStartInfo]::new((Get-Process -Id $PID).Path)
    $info.UseShellExecute = $false
    $info.WorkingDirectory = $root
    # Give the detached launcher its own pipes, so callers can capture the
    # wrapper's output without waiting for the viewer's entire lifetime.
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    foreach ($arg in @('-NoProfile', '-File', "$PSScriptRoot/uml-launch.ps1", $manifest)) {
        $info.ArgumentList.Add($arg)
    }
    $process = [Diagnostics.Process]::Start($info)
    $process.StandardInput.Close()
    Write-Output "UML viewer starting (launcher pid $($process.Id)). Log: $log"
    exit 0
} catch { [Console]::Error.WriteLine("uml: $_"); exit 1 }
