# One guardrail command: the checks that are safe to run at any time.
# Stops at the first failure so the printed label names the actual cause.
# The quality gate (`./uml crap` / `./uml mutate`) is deliberately absent: it
# needs serialized, declared-scope runs against a real project, so it cannot be
# a default check. Native LOVE checks open windows and one launches a real
# companion, hence the explicit -Native opt-in.
param(
    [string]$Lua = 'lua',
    [string]$LuaJit = 'luajit',
    [string]$LuaRocks = '',
    [string]$LuaJitRocks = '',
    [switch]$Native,
    [string]$Love = 'C:/Program Files/LOVE/lovec.exe',
    [string]$CimguiDirectory = '',
    [string]$SarasaFont = ''
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
if (-not $CimguiDirectory) { $CimguiDirectory = Join-Path $repo '.uml-viewer/cimgui-love-1.90-1' }
if (-not $SarasaFont) { $SarasaFont = Join-Path $repo '.uml-viewer/fonts/SarasaMonoSC-Regular.ttf' }
$pwsh = (Get-Process -Id $PID).Path
# Interpreters and fixtures come from the caller's environment; put every
# variable this script touches back exactly as it was found.
$restore = @{}
foreach ($name in @('LUA_PATH', 'LUA_CPATH', 'UML_ENTRY_TEST_TMP')) {
    $restore[$name] = [Environment]::GetEnvironmentVariable($name)
}

function New-Result([string]$Label, [string]$Status, [int]$Code, [string]$Output = '', [string]$Detail = '') {
    [pscustomobject]@{ Label = $Label; Status = $Status; Code = $Code; Output = $Output; Detail = $Detail }
}

# One line per check, so the summary stays skimmable even when a detail carries
# a multi-line diagnostic.
function Format-Line([string]$Status, $Result) {
    $detail = ($Result.Detail -replace '\s+', ' ').Trim()
    if ($detail) { return "$Status $($Result.Label)  $detail" }
    "$Status $($Result.Label)"
}

function Resolve-Interpreter([string]$Command) {
    try { (Get-Command $Command -ErrorAction Stop).Source } catch { $null }
}

# A present interpreter that fails its identity probe is a misconfiguration, not
# a reason to skip: the suite would otherwise run under the wrong ABI silently.
function Test-Busted($Runtime) {
    $env:LUA_PATH = $restore['LUA_PATH']
    $env:LUA_CPATH = $restore['LUA_CPATH']
    if ($Runtime.Tree) {
        $tree = $Runtime.Tree.Replace('\', '/')
        $version = $Runtime.Version
        $env:LUA_PATH = "$tree/share/lua/$version/?.lua;$tree/share/lua/$version/?/init.lua;;"
        $env:LUA_CPATH = "$tree/lib/lua/$version/?.dll;;"
    }
    $executable = Resolve-Interpreter $Runtime.Command
    if (-not $executable) {
        return New-Result $Runtime.Label 'skipped' 0 '' "$($Runtime.Command) not on PATH; pass -$($Runtime.Switch)"
    }
    $identity = & $executable -e $Runtime.Probe 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) {
        return New-Result $Runtime.Label 'failed' $LASTEXITCODE $identity "identity probe failed: $executable"
    }
    $output = & $executable scripts/run_busted.lua love/spec 2>&1 | Out-String
    $code = $LASTEXITCODE
    New-Result $Runtime.Label $(if ($code -eq 0) { 'executed' } else { 'failed' }) $code $output (
        $identity.Trim() + ' ' + $executable)
}

function Test-Entries {
    # Do not leak a rocks tree's LUA_PATH into the Python suite, which spawns
    # PowerShell entries that would inherit it.
    $env:LUA_PATH = $restore['LUA_PATH']
    $env:LUA_CPATH = $restore['LUA_CPATH']
    $python = Resolve-Interpreter 'python'
    if (-not $python) {
        return New-Result 'entries (python)' 'failed' 1 'python is required by the installer and this suite; install Python 3.11+.'
    }
    # Short prefix: the suite copies this repository into the temp root and
    # deep evidence paths under it already reach the Windows MAX_PATH limit.
    $env:UML_ENTRY_TEST_TMP = Join-Path ([IO.Path]::GetTempPath()) ('uml-ck-' + [guid]::NewGuid().ToString().Substring(0, 8))
    $null = New-Item -ItemType Directory -Force -Path $env:UML_ENTRY_TEST_TMP
    $output = & $python -I scripts/test_entries.py 2>&1 | Out-String
    $code = $LASTEXITCODE
    # The suite creates throwaway git fixtures; their CRLF notices repeat for
    # every test and would bury the traceback that actually names the failure.
    # Git prefixes them with unittest's progress dots, so match anywhere.
    $output = ($output -split "`r?`n" | Where-Object { $_ -notmatch 'warning: in the working copy of ' }) -join "`n"
    New-Result 'entries (python -I scripts/test_entries.py)' $(if ($code -eq 0) { 'executed' } else { 'failed' }) $code $output $env:UML_ENTRY_TEST_TMP
}

# Each native check still creates its own temporary project; this evidence dir
# is only where its console output is kept, one per file, so a failure can be
# read without re-running a window-opening test.
function Test-Native([string]$File, [string]$EvidenceRoot) {
    $name = [IO.Path]::GetFileNameWithoutExtension($File)
    $Label = "native $name"
    $evidence = Join-Path $EvidenceRoot $name
    $null = New-Item -ItemType Directory -Force -Path $evidence
    $declared = (Get-Command $File).Parameters.Keys
    $arguments = @('-NoProfile', '-File', $File)
    if ($declared -contains 'Love') { $arguments += @('-Love', $Love) }
    if ($declared -contains 'CimguiDirectory') {
        if (-not (Test-Path -LiteralPath $CimguiDirectory)) {
            return New-Result $Label 'failed' 1 "cimgui bindings missing: $CimguiDirectory (run get-uml-viewer)"
        }
        $arguments += @('-CimguiDirectory', (Resolve-Path $CimguiDirectory).Path)
    }
    if ($declared -contains 'SarasaFont') {
        if (-not (Test-Path -LiteralPath $SarasaFont)) {
            return New-Result $Label 'failed' 1 "font fixture missing: $SarasaFont (run get-uml-viewer)"
        }
        $arguments += @('-SarasaFont', (Resolve-Path $SarasaFont).Path)
    }
    if ($declared -contains 'Lua54') {
        $lua = Resolve-Interpreter $Lua
        if (-not $lua) { return New-Result $Label 'skipped' 0 '' "$Lua not on PATH; pass -Lua" }
        $arguments += @('-Lua54', $lua)
    }
    if ($declared -contains 'LuaJit') {
        $luajit = Resolve-Interpreter $LuaJit
        if (-not $luajit) { return New-Result $Label 'skipped' 0 '' "$LuaJit not on PATH; pass -LuaJit" }
        $arguments += @('-LuaJit', $luajit)
    }
    $output = & $pwsh @arguments 2>&1 | Out-String
    $code = $LASTEXITCODE
    [IO.File]::WriteAllText((Join-Path $evidence 'output.txt'), $output)
    New-Result $Label $(if ($code -eq 0) { 'executed' } else { 'failed' }) $code $output $evidence
}

$results = [Collections.Generic.List[object]]::new()
$luaRuns = 0
try {
    Push-Location $repo
    $checks = @(
        # Probes keep single-quoted Lua strings: a .cmd shim such as a Windows
        # Lua distribution forwards arguments through cmd, which eats the inner
        # double quotes and turns the probe into a syntax error.
        { Test-Busted @{ Label = 'busted (Lua 5.4)'; Command = $Lua; Switch = 'Lua'; Tree = $LuaRocks; Version = '5.4'; Probe = "assert(_VERSION == 'Lua 5.4', 'Lua 5.4 required'); print(_VERSION)" } }
        { Test-Busted @{ Label = 'busted (LuaJIT)'; Command = $LuaJit; Switch = 'LuaJit'; Tree = $LuaJitRocks; Version = '5.1'; Probe = "assert(jit and jit.version, 'LuaJIT required'); print(jit.version)" } }
        { Test-Entries }
    )
    if ($Native) {
        $evidenceRoot = Join-Path ([IO.Path]::GetTempPath()) ('uml-check-native-' + [guid]::NewGuid().ToString('N'))
        foreach ($file in Get-ChildItem -Path (Join-Path $repo 'scripts') -Filter 'test_love_*.ps1' | Sort-Object Name) {
            $path = $file.FullName
            $checks += { Test-Native $path $evidenceRoot }.GetNewClosure()
        }
    }
    foreach ($check in $checks) {
        $result = & $check
        $results.Add($result)
        if ($result.Label -like 'busted*' -and $result.Status -ne 'skipped') { $luaRuns++ }
        if ($result.Status -eq 'failed') { break }
    }
    # A run with no Lua interpreter at all verified nothing; that is fatal even
    # though each individual interpreter is only a skip.
    if ($luaRuns -eq 0) {
        $results.Add((New-Result 'lua interpreters' 'failed' 1 "Neither $Lua nor $LuaJit resolved; the pure suite did not run."))
    }
} finally {
    foreach ($name in $restore.Keys) { [Environment]::SetEnvironmentVariable($name, $restore[$name]) }
    Pop-Location
}

$counts = @{ passed = 0; skipped = 0; failed = 0 }
foreach ($result in $results) {
    switch ($result.Status) {
        'executed' { $counts.passed++; Format-Line 'PASS' $result }
        'skipped' { $counts.skipped++; Format-Line 'SKIP' $result }
        'failed' {
            $counts.failed++
            Format-Line 'FAIL' $result
            if ($result.Output) { $result.Output.TrimEnd() }
        }
    }
}
'note: ./uml crap and ./uml mutate are not run here; the quality gate needs serialized, declared-scope runs.'
"check: $($counts.passed) passed, $($counts.skipped) skipped, $($counts.failed) failed"
if ($counts.failed -gt 0) { exit 1 }
exit 0
