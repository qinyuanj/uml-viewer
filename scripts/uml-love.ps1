# Isolated LOVE viewer entry. Companion selection, identity and same-runtime
# --restart are handled here; the Herdr preflight runs in the LOVE runtime.
function Get-UmlShortHead($path) {
    try {
        $sha = & git -C $path rev-parse --short HEAD 2>$null
        if ($LASTEXITCODE -eq 0 -and $sha) { return ([string]$sha).Trim() }
    } catch { }
    return $null
}

function Resolve-UmlToolRoot($projectRoot) {
    $override = $env:UML_VIEWER_TOOL_ROOT
    if ($override) {
        $resolved = Resolve-Path -LiteralPath $override -ErrorAction SilentlyContinue
        if (-not $resolved -or -not (Test-Path -LiteralPath (Join-Path $resolved.Path 'love/main.lua'))) {
            throw "UML_VIEWER_TOOL_ROOT must name an uml-viewer checkout containing love/main.lua: $override"
        }
        $toolRoot = $resolved.Path.Replace('\', '/')
    } else {
        $toolRoot = (Resolve-Path -LiteralPath '.uml-viewer/uml-viewer').Path.Replace('\', '/')
    }
    # A pinned installed clone silently launches stale code; flag it instead of
    # failing, and only when both roots are git checkouts that can be compared.
    $workingTree = [IO.Path]::GetFullPath($projectRoot).TrimEnd('\', '/').Replace('\', '/')
    if (-not [string]::Equals($toolRoot, $workingTree, [StringComparison]::OrdinalIgnoreCase)) {
        $toolHead = Get-UmlShortHead $toolRoot
        $treeHead = Get-UmlShortHead $workingTree
        if ($toolHead -and $treeHead -and $toolHead -ne $treeHead) {
            [Console]::Error.WriteLine("uml: tool root $toolRoot is at $toolHead, working tree is $treeHead; run with UML_VIEWER_TOOL_ROOT=$workingTree to use this checkout.")
        }
    }
    return $toolRoot
}

function Get-UmlLoveConsoleHint($love) {
    $name = [IO.Path]::GetFileNameWithoutExtension($love.Source)
    if ($name -like 'love*' -and $name -notlike 'lovec*') {
        return "$($love.Source) emitted no stdout; a GUI-subsystem LOVE build cannot serve as the preflight source. Set UML_LOVE_EXECUTABLE to lovec.exe."
    }
    return ''
}

function Invoke-UmlLove($projectRoot, $toolRoot, $viewerArgs) {
    if (-not $IsWindows) { throw 'LOVE viewer tracer currently supports native Windows only.' }
    if ($viewerArgs -contains '--restart') {
        # Validate the saved runtime before running the read-only backend
        # preflight or waiting for the exact old LOVE process.
        . "$PSScriptRoot/uml-process-identity.ps1"
        $owner = Get-UmlLauncherOwner $projectRoot
        $null = Assert-UmlRestartRuntime $projectRoot $owner.Record 'love'
        # Validate the identity (and refuse a changed live process) before the
        # potentially long launcher wait.
        Wait-UmlLoveIdentity $projectRoot
    }
    if (-not $env:UML_CIMGUI_DIRECTORY) {
        $installed = Join-Path $projectRoot '.uml-viewer/cimgui-love-1.90-1'
        if (Test-Path -LiteralPath "$installed/cimgui/init.lua") { $env:UML_CIMGUI_DIRECTORY = $installed }
    }
    $candidate = if ($env:UML_LOVE_EXECUTABLE) { $env:UML_LOVE_EXECUTABLE } else { 'love' }
    $love = Get-Command $candidate -ErrorAction SilentlyContinue
    if (-not $love -or $love.CommandType -notin @('Application', 'ExternalScript')) {
        $hint = ''
        if ($env:UML_LOVE_EXECUTABLE) {
            $hint = " UML_LOVE_EXECUTABLE=$($env:UML_LOVE_EXECUTABLE) could not be resolved; set it to the console build lovec.exe."
        }
        throw "LOVE 11.5 is required. Install https://love2d.org/ and put love.exe on PATH (or set UML_LOVE_EXECUTABLE).$hint"
    }
    if ([IO.Path]::GetExtension($love.Source).ToLowerInvariant() -in @('.cmd', '.bat')) { throw 'LOVE must be a native executable or a PowerShell script entry.' }
    if ($viewerArgs.Count -gt 0 -and $viewerArgs[0] -eq 'ir') {
        $name = Split-Path $projectRoot -Leaf
        $policy = if (Test-Path -LiteralPath "examples/$name.policy.edn") { "examples/$name.policy.edn" } else {
            $first = Get-ChildItem 'examples/*.policy.edn' -ErrorAction SilentlyContinue | Sort-Object Name | Select-Object -First 1
            if ($first) { $first.FullName } elseif (Test-Path 'policy.edn') { 'policy.edn' }
        }
        if (-not $policy) { throw "no policy. Write examples/$name.policy.edn." }
        $forward = @($viewerArgs | Select-Object -Skip 1)
        & $love.Source (Join-Path $toolRoot 'love') ir $policy @forward
        return
    }
    # One short-lived LOVE process performs read-only Herdr selection. It loads
    # no viewer, creates no owner/server, and never starts a JVM. Native stderr
    # carries the selection diagnostic; it must not be merged into the one-line
    # stdout contract the wrapper validates.
    $global:LASTEXITCODE = 0
    $preflightError = [IO.Path]::GetTempFileName()
    try {
        $checked = @(& $love.Source (Join-Path $toolRoot 'love') companion-preflight $projectRoot @viewerArgs 2> $preflightError)
        $preflightExit = $LASTEXITCODE
        $diagnostic = Get-Content -LiteralPath $preflightError -Raw -ErrorAction SilentlyContinue
    } finally {
        Remove-Item -LiteralPath $preflightError -Force -ErrorAction SilentlyContinue
    }
    if ($preflightExit -ne 0) {
        if (-not $diagnostic) { $diagnostic = 'LOVE companion preflight failed without diagnostic.' }
        throw $diagnostic.Trim()
    }
    if ($diagnostic) { [Console]::Error.Write($diagnostic) }
    $lines = @($checked | Where-Object { $null -ne $_ } | ForEach-Object { ([string]$_).Trim() } | Where-Object { $_ -ne '' })
    if ($lines.Count -ne 1 -or $lines[0] -cne 'herdr') {
        $message = 'Backend preflight returned invalid output; no viewer was launched.'
        if ($lines.Count -eq 0) {
            $hint = Get-UmlLoveConsoleHint $love
            if ($hint) { $message = "$message $hint" }
        }
        throw $message
    }
    # Strip only the exact option pair the preflight consumed, preserving all
    # other argv literally, then forward explicit Herdr selection to the viewer.
    $forwarded = @()
    for ($i = 0; $i -lt $viewerArgs.Count; $i++) {
        if ($viewerArgs[$i] -ceq '--backend') { $i++ } else { $forwarded += $viewerArgs[$i] }
    }
    $viewerArgs = @('--backend', 'herdr') + $forwarded
    if ($forwarded -contains '--restart') {
        # Preflight is read-only, so the old process is still verified above and
        # waited for only now, immediately before replacement.
        Wait-UmlLauncherOwner $owner 'LOVE viewer'
    }
    $directory = Join-Path $projectRoot '.uml-viewer'
    $null = New-Item -ItemType Directory -Force -Path $directory
    $manifest = Join-Path $directory ('launch-' + [guid]::NewGuid().ToString('N') + '.json')
    $log = Join-Path $projectRoot 'uml-viewer-log.txt'
    @{ executable = $love.Source; powershell = ($love.CommandType -eq 'ExternalScript'); module = $null;
       cwd = $projectRoot; log = $log; runtime = 'love'; argv = @((Join-Path $toolRoot 'love'), 'viewer') + @($viewerArgs) } |
        ConvertTo-Json -Depth 4 -Compress | Set-Content -LiteralPath $manifest -Encoding utf8
    $info = [Diagnostics.ProcessStartInfo]::new((Get-Process -Id $PID).Path)
    $info.UseShellExecute = $false
    $info.WorkingDirectory = $projectRoot
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    foreach ($value in @('-NoProfile', '-File', "$PSScriptRoot/uml-launch.ps1", $manifest)) { $info.ArgumentList.Add($value) }
    $process = [Diagnostics.Process]::Start($info)
    $process.StandardInput.Close()
    Write-Output "UML LOVE viewer starting (launcher pid $($process.Id)). Log: $log"
    $global:LASTEXITCODE = 0
}
