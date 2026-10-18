# Shared restart identity checks. Read-only: a caller validates everything here
# before writing a launch manifest, replacing ownership, or starting a process.
$script:UmlFiletimeEpochTicks = [bigint]504911232000000000

function Get-UmlLauncherOwner {
    param([Parameter(Mandatory = $true)][string]$Root)
    $recordPath = Join-Path $Root '.uml-viewer/viewer-process.json'
    if (-not (Test-Path -LiteralPath $recordPath)) {
        throw 'No launcher process record; launch through ./uml before restarting.'
    }
    $record = Get-Content -LiteralPath $recordPath -Raw | ConvertFrom-Json
    if (-not $record.pid -or -not $record.started) {
        throw 'Invalid launcher process record; cannot safely restart.'
    }
    $process = Get-Process -Id $record.pid -ErrorAction SilentlyContinue
    if ($process -and $process.StartTime.ToUniversalTime().Ticks.ToString() -cne [string]$record.started) {
        throw 'Launcher process identity changed; cannot safely restart.'
    }
    return [pscustomobject]@{ Record = $record; Process = $process; Path = $recordPath }
}

function Get-UmlRuntimeProvenance {
    param($Root, $LauncherRecord)
    # The launcher record is written for every new launch and names the runtime
    # that owns the current session, so stale records from the other runtime do
    # not make an otherwise unambiguous restart ambiguous.
    $declared = if ($null -ne $LauncherRecord) { $LauncherRecord.runtime } else { $null }
    if ($null -ne $declared -and [string]$declared -ne '') {
        $value = [string]$declared
        if ($value -cnotin @('jvm', 'love')) { throw "Invalid viewer runtime '$value'; cannot safely restart." }
        return $value
    }
    $jvm = Test-Path -LiteralPath (Join-Path $Root '.uml-viewer/viewer-jvm.json')
    $love = Test-Path -LiteralPath (Join-Path $Root '.uml-viewer/viewer-process-identity.json')
    if ($jvm -and $love) { return 'ambiguous' }
    if ($jvm) { return 'jvm' }
    if ($love) { return 'love' }
    return $null
}

function Assert-UmlRestartRuntime {
    param([string]$Root, $LauncherRecord, [Parameter(Mandatory = $true)][string]$Expected)
    $actual = Get-UmlRuntimeProvenance $Root $LauncherRecord
    if ($actual -ceq 'ambiguous') {
        throw 'Viewer runtime identity is ambiguous (both JVM and LOVE process records); cannot safely restart.'
    }
    if ($null -ne $actual -and $actual -cne $Expected) {
        $label = if ($actual -ceq 'jvm') { 'JVM' } else { 'LOVE' }
        throw "Cannot restart a $label viewer as $Expected; use --runtime $actual."
    }
    return $actual
}

function Wait-UmlLauncherOwner {
    param($Owner, [Parameter(Mandatory = $true)][string]$Label)
    if (-not $Owner.Process) { return }
    Write-Output "Waiting for the old $Label after :quit-for-restart..."
    if (-not $Owner.Process.WaitForExit(30000)) {
        throw 'Old viewer is still running. Send :quit-for-restart, wait for its exit, then retry ./uml --restart.'
    }
}

function Wait-UmlLoveIdentity {
    param([Parameter(Mandatory = $true)][string]$Root)
    $recordPath = Join-Path $Root '.uml-viewer/viewer-process-identity.json'
    if (-not (Test-Path -LiteralPath $recordPath)) {
        throw 'No LOVE viewer process identity; cannot safely restart without authoritative LOVE identity.'
    }
    $record = Get-Content -LiteralPath $recordPath -Raw | ConvertFrom-Json
    $ownerPid = 0
    if (-not [int]::TryParse([string]$record.pid, [ref]$ownerPid) -or $ownerPid -le 0 -or
        [string]$record.runtime -cne 'love' -or [string]$record.started -notmatch '^\d+$') {
        throw 'Invalid LOVE viewer process identity; cannot safely restart.'
    }
    $recordedStart = [bigint]::Parse([string]$record.started, [Globalization.CultureInfo]::InvariantCulture)
    $process = Get-Process -Id $ownerPid -ErrorAction SilentlyContinue
    if (-not $process) { return }
    try { $actualStart = [bigint]$process.StartTime.ToUniversalTime().Ticks }
    catch { return }
    # GetProcessTimes reports 100ns FILETIME ticks since 1601; .NET DateTime
    # ticks run from 0001. Subtracting the fixed 1601 .NET base is exact and
    # keeps the full creation precision (no millisecond tolerance).
    $actualStart = $actualStart - $script:UmlFiletimeEpochTicks
    if ($process.Name -notmatch '^(love|lovec)$' -or $actualStart -ne $recordedStart) {
        throw 'LOVE viewer process identity changed; cannot safely restart.'
    }
    Write-Output 'Waiting for the old LOVE viewer after :quit-for-restart...'
    if (-not $process.WaitForExit(30000)) {
        throw 'Old LOVE viewer is still running. Send :quit-for-restart, wait for its exit, then retry ./uml --restart.'
    }
}
