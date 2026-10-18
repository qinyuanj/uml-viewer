# PowerShell 7 installer. Run from the project being examined.
if ($PSVersionTable.PSVersion.Major -lt 7) {
    Write-Error 'PowerShell 7 is required: https://aka.ms/powershell-install'; exit 1
}
$ErrorActionPreference = 'Stop'
function Fetch-Repo($Directory, $Url, $Ref) {
    if (-not (Test-Path "$Directory/.git")) {
        if (Test-Path $Directory) { throw "Refusing to replace non-git directory: $Directory" }
        & git init -q $Directory
        if ($LASTEXITCODE -ne 0) { throw "git init failed: $Directory" }
    }
    & git -C $Directory fetch --depth 1 $Url $Ref
    if ($LASTEXITCODE -ne 0) { throw "git fetch failed: $Directory" }
    & git -C $Directory checkout -q FETCH_HEAD
    if ($LASTEXITCODE -ne 0) { throw "git checkout failed: $Directory" }
}
function Setting($Name, $Default) {
    $value = [Environment]::GetEnvironmentVariable($Name)
    if ($value) { return $value }; return $Default
}
function Get-LoveCommand {
    $candidate = Setting 'UML_LOVE_EXECUTABLE' 'love'
    $command = Get-Command $candidate -ErrorAction SilentlyContinue
    if (-not $command -or $command.CommandType -notin @('Application', 'ExternalScript')) {
        throw 'LOVE 11.5 is required. Install https://love2d.org/ and put love.exe on PATH (or set UML_LOVE_EXECUTABLE).'
    }
    if ([IO.Path]::GetExtension($command.Source).ToLowerInvariant() -in @('.cmd', '.bat')) {
        throw 'LOVE must be a native executable or a PowerShell script entry.'
    }
    $version = @(& $command.Source --version 2>&1)
    $text = ($version | ForEach-Object { ([string]$_).Trim() } | Where-Object { $_ }) -join "`n"
    if ($LASTEXITCODE -ne 0 -or $text -notmatch 'LOVE (\d+)\.(\d+)') {
        throw "LOVE 11.5 is required; '$($command.Source) --version' did not report a LOVE release."
    }
    $major = [int]$Matches[1]; $minor = [int]$Matches[2]
    if ($major -lt 11 -or ($major -eq 11 -and $minor -lt 5)) {
        throw "LOVE 11.5 or newer is required; found $major.$minor."
    }
    return $command
}
function Assert-Cimgui([string]$LovePath) {
    $directory = Setting 'UML_CIMGUI_DIRECTORY' ''
    if (-not $directory) { $directory = Split-Path $LovePath -Parent }
    $dll = Join-Path $directory 'cimgui.dll'
    $lua = Join-Path $directory 'cimgui.lua'
    if (-not (Test-Path -LiteralPath $lua)) { $lua = Join-Path $directory 'cimgui/init.lua' }
    if (-not (Test-Path -LiteralPath $dll) -or -not (Test-Path -LiteralPath $lua)) {
        throw 'The cimgui-love dynamic libraries are required beside LOVE: cimgui.dll and cimgui.lua or cimgui/init.lua. Extract the cimgui-love release and set UML_CIMGUI_DIRECTORY to that directory.'
    }
}
function Assert-Herdr {
    $command = Get-Command herdr -ErrorAction SilentlyContinue
    if (-not $command -or $command.CommandType -notin @('Application', 'ExternalScript')) {
        throw 'Herdr >= 0.9.3 is required. Install a release from https://github.com/herdrdev/herdr/releases and put herdr on PATH.'
    }
    $version = @(& $command.Source --version 2>&1)
    $text = ($version | ForEach-Object { ([string]$_).Trim() } | Where-Object { $_ }) -join "`n"
    if ($LASTEXITCODE -ne 0 -or $text -notmatch '^herdr (\d+)\.(\d+)\.(\d+)$') {
        throw "Herdr >= 0.9.3 is required; '$($command.Source) --version' did not report herdr X.Y.Z."
    }
    $found = @([int]$Matches[1], [int]$Matches[2], [int]$Matches[3])
    $floor = @(0, 9, 3)
    for ($index = 0; $index -lt 3; $index++) {
        if ($found[$index] -gt $floor[$index]) { return }
        if ($found[$index] -lt $floor[$index]) { break }
    }
    if (($found[0] -eq $floor[0]) -and ($found[1] -eq $floor[1]) -and ($found[2] -eq $floor[2])) { return }
    throw "Herdr >= 0.9.3 is required; found $($found -join '.')."
}
function Get-PythonCommand {
    $override = Setting 'UML_PYTHON_EXECUTABLE' ''
    $candidates = if ($override) { @(@{ Name = $override; Prefix = $null }) } else {
        @(@{ Name = 'py'; Prefix = '-3' }, @{ Name = 'python'; Prefix = $null })
    }
    foreach ($candidate in $candidates) {
        $command = Get-Command $candidate.Name -ErrorAction SilentlyContinue
        if (-not $command -or $command.CommandType -notin @('Application', 'ExternalScript')) { continue }
        $prefixArgs = @(); if ($candidate.Prefix) { $prefixArgs += $candidate.Prefix }
        $probe = @(& $command.Source @prefixArgs '-c' 'import sys; print(sys.version_info[0]*100+sys.version_info[1])' 2>&1)
        if ($LASTEXITCODE -eq 0) {
            $text = ($probe | Select-Object -Last 1)
            if ($null -ne $text) { $text = ([string]$text).Trim() }
            if ($text -match '^\d+$' -and [int]$text -ge 311) {
                return @{ Exe = $command.Source; Prefix = $candidate.Prefix }
            }
        }
    }
    throw 'Python 3.11+ is required. Install https://www.python.org/downloads/ (or set UML_PYTHON_EXECUTABLE).'
}
function Install-ToolVenv([string]$Tool, $Python) {
    $directory = ".uml-viewer/$Tool"
    $venv = "$directory/.venv"
    $venvPython = if ($IsWindows) { "$venv/Scripts/python.exe" } else { "$venv/bin/python" }
    if (-not (Test-Path -LiteralPath $venvPython)) {
        $prefixArgs = @(); if ($Python.Prefix) { $prefixArgs += $Python.Prefix }
        & $Python.Exe @prefixArgs -m venv $venv
        if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $venvPython)) {
            throw "Could not create $venv with Python 3.11+."
        }
    }
    & $venvPython -m pip install --disable-pip-version-check -e $directory
    if ($LASTEXITCODE -ne 0) { throw "Could not install $Tool into $venv." }
}
try {
    if ($args.Count -eq 1 -and $args[0] -in @('-h', '--help')) {
        Write-Output 'usage: get-uml-viewer [--install-only]'; exit 0
    }
    if ($args.Count -gt 1 -or ($args.Count -eq 1 -and $args[0] -ne '--install-only')) {
        throw 'usage: get-uml-viewer [--install-only]'
    }
    if (-not (Get-Command git -ErrorAction SilentlyContinue)) {
        throw 'git is required. Install Git: https://git-scm.com/downloads'
    }
    $love = Get-LoveCommand
    Assert-Cimgui $love.Source
    Assert-Herdr
    $python = Get-PythonCommand
    $root = (Get-Location).Path
    New-Item -ItemType Directory -Force '.uml-viewer' | Out-Null
    Fetch-Repo '.uml-viewer/uml-viewer' (Setting 'UML_VIEWER_REPO_URL' 'https://github.com/qinyuanj/uml-viewer.git') (Setting 'UML_VIEWER_REF' 'lua')
    # Both quality tools are mandatory for every project, independent of any
    # deps.edn left over from the removed Clojure implementation.
    $toolErrors = @()
    foreach ($tool in @('crapper', 'mutator')) {
        try {
            Fetch-Repo ".uml-viewer/$tool" (Setting "$($tool.ToUpper())_REPO_URL" "https://github.com/qinyuanj/$tool.git") (Setting "$($tool.ToUpper())_REF" 'lua')
        } catch { $toolErrors += "${tool}: $_" }
    }
    if ($toolErrors.Count -gt 0) { throw ('Could not fetch required quality tools: ' + ($toolErrors -join '; ')) }
    foreach ($tool in @('crapper', 'mutator')) { Install-ToolVenv $tool $python }
    $ignore = if (Test-Path '.gitignore') { [string](Get-Content '.gitignore' -Raw) } else { '' }
    $ignore = [regex]::Replace($ignore, '(?ms)^# BEGIN UML-VIEWER\r?\n.*?^# END UML-VIEWER\r?\n?', '')
    [IO.File]::WriteAllText((Join-Path $root '.gitignore'), $ignore.TrimEnd() + "`n# BEGIN UML-VIEWER`n.uml-viewer/`numl-viewer-log.txt`n# END UML-VIEWER`n")
    foreach ($name in @('uml', 'uml.ps1')) {
        Copy-Item ".uml-viewer/uml-viewer/scripts/$name" (Join-Path $root $name) -Force
    }
    # Earlier installers also wrote a uml.cmd shim. Windows now runs uml.ps1
    # through pwsh directly; remove a stale shim only when its content is the
    # one this installer generated, so a user's own uml.cmd is never touched.
    $stale = Join-Path $root 'uml.cmd'
    if ((Test-Path -LiteralPath $stale) -and
        ((Get-Content -LiteralPath $stale -Raw) -match '-NoProfile -File "%~dp0uml\.ps1"')) {
        Remove-Item -LiteralPath $stale -Force
    }
    if (-not $IsWindows) {
        & chmod +x (Join-Path $root 'uml')
        if ($LASTEXITCODE -ne 0) { throw 'Could not make ./uml executable' }
    }
    Write-Output 'Installed uml and uml.ps1. Run ./uml (Windows: pwsh -File .\uml.ps1) to start.'
    if ($args.Count -eq 0) { & (Join-Path $root 'uml.ps1'); exit $LASTEXITCODE }
    exit 0
} catch { [Console]::Error.WriteLine("get-uml-viewer: $_"); exit 1 }
