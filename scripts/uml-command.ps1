# Shared installed command runtime. Root is supplied by the thin project entry.
if ($PSVersionTable.PSVersion.Major -lt 7) {
    Write-Error 'PowerShell 7 is required: https://aka.ms/powershell-install'; exit 1
}
$ErrorActionPreference = 'Stop'
$root = $args[0]
$commandArgs = @($args | Select-Object -Skip 1)
try {
    Set-Location -LiteralPath $root
    $runtime = 'love'
    $remaining = @()
    $runtimeSeen = $false
    for ($i = 0; $i -lt $commandArgs.Count; $i++) {
        if ($commandArgs[$i] -ceq '--runtime') {
            if ($runtimeSeen -or $i + 1 -ge $commandArgs.Count) { throw '--runtime requires exactly one love value.' }
            $runtimeSeen = $true
            $value = $commandArgs[++$i]
            if ($value -ceq 'jvm') { throw 'The JVM runtime was removed in the LOVE cutover; --runtime love is the only runtime.' }
            if ($value -cne 'love') { throw '--runtime requires exactly one love value.' }
            $runtime = $value
        } else { $remaining += $commandArgs[$i] }
    }
    $commandArgs = $remaining
    if ($commandArgs.Count -gt 0 -and $commandArgs[0] -in @('-h', '--help')) {
        Write-Output 'usage: uml [--runtime love] [--backend herdr] [--restart | ir | crap | mutate] [args]'; exit 0
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
        $declared = $null -ne $standard
        if (-not $declared) {
            # No declaration: standard tools inspect the whole project. The
            # Clojure alias route was removed with the JVM implementation.
            $standard = [pscustomobject]@{ sourceRoots = @(); exclude = @(); crapperOptions = @(); mutatorOptions = @() }
        } else {
            if (@($forward | Where-Object { $_ -in @('--root', '--source-root', '-s', '--exclude') }).Count -gt 0) {
                throw 'Standard scope is configured in standardTools; refresh arguments cannot override roots or exclusions.'
            }
            $null = @(Get-UmlBusinessRoots $root $standard)
        }
        . "$PSScriptRoot/uml-standard-quality.ps1"
        if ($alias -eq 'mutate' -and $declared) { $forward = @(Select-UmlMutationTargets $root $standard $forward) }
        if ($alias -eq 'crap') {
            $scopeArgs = @()
            foreach ($sourceRoot in $standard.sourceRoots) { $scopeArgs += @('--source-root', $sourceRoot) }
            foreach ($exclusion in $standard.exclude) { $scopeArgs += @('--exclude', $exclusion) }
            $forward = $scopeArgs + @($standard.crapperOptions) + $forward
        } else {
            # Mutator prioritizes --source-root over explicit files; the adapter
            # already applied roots, so pass only filtered targets.
            $forward = @($standard.mutatorOptions) + $forward
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
                throw "$python is missing. Install Python 3.11+ (https://www.python.org/downloads/), re-run get-uml-viewer to fetch and provision the tool, or run: py -3 -m venv `"$directory/.venv`"; & `"$python`" -m pip install -e `"$directory`"."
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
    . "$PSScriptRoot/uml-love.ps1"
    $toolRoot = Resolve-UmlToolRoot $root
    Invoke-UmlLove $root $toolRoot $commandArgs
    exit $LASTEXITCODE
} catch { [Console]::Error.WriteLine("uml: $_"); exit 1 }
