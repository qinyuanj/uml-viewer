# Isolated LOVE viewer entry. Companion integration belongs to #70; runtime
# identity and same-runtime --restart are handled here (see #69).
function Invoke-UmlLove($projectRoot, $toolRoot, $viewerArgs) {
    if (-not $IsWindows) { throw 'LOVE viewer tracer currently supports native Windows only.' }
    if ($viewerArgs -contains '--backend') { throw 'LOVE companion integration is not available yet (#70); omit --backend.' }
    if ($viewerArgs -contains '--restart') {
        # Validate the saved runtime and wait for the exact old LOVE process
        # before writing a manifest or creating any replacement ownership.
        . "$PSScriptRoot/uml-process-identity.ps1"
        $owner = Get-UmlLauncherOwner $projectRoot
        $null = Assert-UmlRestartRuntime $projectRoot $owner.Record 'love'
        # Validate the identity (and refuse a changed live process) before the
        # potentially long launcher wait, then wait for both exact processes.
        Wait-UmlLoveIdentity $projectRoot
        Wait-UmlLauncherOwner $owner 'LOVE viewer'
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
    if ($viewerArgs.Count -gt 0 -and $viewerArgs[0] -in @('crap', 'mutate')) { throw '--runtime love currently selects viewer or ir; quality dispatch uses the default runtime.' }
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
    Write-Output "UML LOVE viewer starting (launcher pid $($process.Id)); companion integration pending #70. Log: $log"
    $global:LASTEXITCODE = 0
}
