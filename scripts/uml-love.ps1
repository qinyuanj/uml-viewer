# Isolated LOVE viewer entry. Companion selection, identity and same-runtime
# --restart are handled here; the Herdr preflight runs in the LOVE runtime.
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
        throw 'LOVE 11.5 is required. Install https://love2d.org/ and put love.exe on PATH (or set UML_LOVE_EXECUTABLE).'
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
        throw 'Backend preflight returned invalid output; no viewer was launched.'
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
