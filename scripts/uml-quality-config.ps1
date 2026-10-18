# Optional repository-owned dispatch. Dot-sourced by the shared public wrapper.
. "$PSScriptRoot/uml-standard-quality.ps1"
function Read-UmlQualityConfig([string]$ProjectRoot) {
    $path = Join-Path $ProjectRoot 'uml-quality.json'
    if (-not (Test-Path -LiteralPath $path)) { return $null }
    try { $config = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json -AsHashtable }
    catch { throw "Invalid uml-quality.json: $_" }
    if ($config -isnot [System.Collections.IDictionary]) { throw 'Invalid uml-quality.json: expected an object.' }
    if ($config.ContainsKey('aliasResultExitCodes')) {
        throw 'Invalid uml-quality.json: aliasResultExitCodes belongs to the removed Clojure runtime; declare standardTools instead.'
    }
    if (($config.version -isnot [long] -and $config.version -isnot [int]) -or $config.version -ne 1 -or
        (-not $config.ContainsKey('standardTools') -and -not $config.ContainsKey('operations'))) {
        throw 'Invalid uml-quality.json: version 1 and standardTools or operations are required. The Clojure alias route was removed.'
    }
    if ($config.ContainsKey('standardTools')) {
        $standard = $config.standardTools
        if ($standard -isnot [System.Collections.IDictionary]) { throw 'standardTools must be an object.' }
        foreach ($key in $standard.Keys) {
            if ($key -cnotin @('sourceRoots', 'exclude', 'crapperOptions', 'mutatorOptions') -or
                $standard[$key] -isnot [array] -or
                @($standard[$key] | Where-Object { $_ -isnot [string] -or [string]::IsNullOrWhiteSpace($_) }).Count -gt 0) {
                throw "Invalid standardTools.${key}: expected an array of nonempty strings."
            }
        }
        foreach ($optionKey in @('crapperOptions', 'mutatorOptions')) {
            $needsValue = $false
            $valueOptions = @('--coverage-command', '--threshold', '--test-command', '--busted-task', '--timeout-factor', '--mutation-warning', '--max-workers', '--lines')
            foreach ($option in $standard[$optionKey]) {
                if ($needsValue) { $needsValue = $false; continue }
                if (-not $option.StartsWith('--')) { throw "$optionKey accepts tool options, not positional targets." }
                $needsValue = $option -in $valueOptions
                if ($option -in @('--root', '--source-root', '-s', '--exclude', '--full-tree', '--changed', '--mutate-all')) {
                    throw "standardTools.$optionKey cannot declare routing flag $option. Use scope fields or explicit refresh arguments for --mutate-all."
                }
            }
            if ($needsValue) { throw "$optionKey has an option missing its value." }
        }
        $null = @(Get-UmlBusinessRoots $ProjectRoot $standard)
    }
    if (-not $config.ContainsKey('operations')) { return $config }
    if ($config.operations -isnot [System.Collections.IDictionary]) { throw 'operations must be an object.' }
    foreach ($name in @('crap', 'mutate')) {
        $op = $config.operations[$name]
        if ($op -isnot [System.Collections.IDictionary] -or $op.argv -isnot [array] -or $op.argv.Count -eq 0 -or
            $op.argv[0] -isnot [string] -or [string]::IsNullOrWhiteSpace($op.argv[0]) -or
            @($op.argv | Where-Object { $_ -isnot [string] }).Count -gt 0 -or
            $op.forwardArgs -cnotin @('append', 'none') -or $op.validResultExitCodes -isnot [array] -or
            $op.validResultExitCodes.Count -eq 0) {
            throw "Invalid uml-quality.json: $name requires argv, forwardArgs and validResultExitCodes."
        }
        foreach ($code in $op.validResultExitCodes) {
            if (($code -isnot [long] -and $code -isnot [int]) -or $code -lt 0 -or $code -gt 255) {
                throw "Invalid uml-quality.json: $name exit codes must be integers from 0 to 255."
            }
        }
        if ($name -eq 'crap' -and $op.scope -cnotin @('targets', 'whole-tree')) {
            throw 'Invalid uml-quality.json: crap scope must be targets or whole-tree.'
        }
        if ($name -eq 'crap' -and $op.scope -ceq 'whole-tree' -and $op.forwardArgs -cne 'none') {
            throw 'Invalid uml-quality.json: whole-tree CRAP must use forwardArgs none.'
        }
    }
    return $config
}

function Write-UmlQualityResult([string]$ProjectRoot, [string]$Operation, $Result) {
    $directory = Join-Path $ProjectRoot '.uml-viewer'
    $record = @{ version = 1; operation = $Operation; projectRoot = $ProjectRoot;
                 exitCode = $Result.exitCode; validResult = $Result.validResult;
                 crapScope = $Result.crapScope; configured = $Result.configured;
                 tool = $(if ($Result.configured) { 'configured' } else { $Result.tool }) }
    $temporary = Join-Path $directory ('quality-result-' + [guid]::NewGuid().ToString('N') + '.tmp')
    try {
        $null = New-Item -ItemType Directory -Force -Path $directory
        $record | ConvertTo-Json -Compress | Set-Content -LiteralPath $temporary -Encoding utf8
        Move-Item -LiteralPath $temporary -Destination (Join-Path $directory 'quality-result.json') -Force
    } catch {
        [Console]::Error.WriteLine("uml: cannot publish quality result: $_")
    } finally {
        if (Test-Path -LiteralPath $temporary) { Remove-Item -LiteralPath $temporary -Force -ErrorAction SilentlyContinue }
    }
}

function Invoke-UmlConfiguredQuality([string]$ProjectRoot, [string]$Operation, [array]$Forward, $Config) {
    $op = $Config.operations[$Operation]
    $executable = $op.argv[0]
    $argv = @($op.argv | Select-Object -Skip 1)
    if ($op.forwardArgs -ceq 'append') { $argv += $Forward }
    $info = [Diagnostics.ProcessStartInfo]::new()
    $info.UseShellExecute = $false
    $info.WorkingDirectory = $ProjectRoot
    # .ps1 is data passed to a fresh PS7 process, never evaluated as shell text.
    if ([IO.Path]::GetExtension($executable) -ieq '.ps1') {
        $info.FileName = (Get-Process -Id $PID).Path
        foreach ($arg in @('-NoProfile', '-File', $executable)) { $info.ArgumentList.Add($arg) }
    } else {
        if ($IsWindows -and [IO.Path]::GetExtension($executable) -iin @('.cmd', '.bat')) {
            throw 'Configured .cmd/.bat commands cannot preserve argv. Use an executable or .ps1 entry.'
        }
        $info.FileName = $executable
    }
    foreach ($arg in $argv) { $info.ArgumentList.Add([string]$arg) }
    $process = [Diagnostics.Process]::Start($info)
    $process.WaitForExit()
    $code = $process.ExitCode
    $process.Dispose()
    return @{ exitCode = $code; validResult = ($op.validResultExitCodes -contains $code);
              crapScope = $(if ($Operation -eq 'crap') { $op.scope } else { $null }); configured = $true }
}
