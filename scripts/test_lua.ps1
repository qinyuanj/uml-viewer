# Native local checks; install busted for each interpreter's ABI first.
# Explicit paths/tree overrides support installations outside PATH. Never share
# native Lua 5.4 DLLs with LuaJIT's Lua 5.1 ABI.
param(
    [string]$LuaJit = 'luajit',
    [string]$Lua54 = 'lua',
    [string]$LuaJitRocks = '',
    [string]$Lua54Rocks = '',
    [string[]]$BustedArgs = @('love/spec')
)
$ErrorActionPreference = 'Stop'
$previousPath = $env:LUA_PATH
$previousCPath = $env:LUA_CPATH
$failures = 0
Push-Location (Split-Path $PSScriptRoot -Parent)
try {
    foreach ($runtime in @(
        @{ Name = 'LuaJIT'; Command = $LuaJit; Tree = $LuaJitRocks; Version = '5.1'; Probe = 'assert(jit and jit.version, "LuaJIT required"); print(jit.version)' },
        @{ Name = 'Lua 5.4'; Command = $Lua54; Tree = $Lua54Rocks; Version = '5.4'; Probe = 'assert(_VERSION == "Lua 5.4", "Lua 5.4 required"); print(_VERSION)' }
    )) {
        try {
            $env:LUA_PATH = $previousPath
            $env:LUA_CPATH = $previousCPath
            if ($runtime.Tree) {
                $tree = $runtime.Tree.Replace('\', '/')
                $version = $runtime.Version
                $env:LUA_PATH = "$tree/share/lua/$version/?.lua;$tree/share/lua/$version/?/init.lua;;"
                $env:LUA_CPATH = "$tree/lib/lua/$version/?.dll;;"
            }
            $executable = (Get-Command $runtime.Command -ErrorAction Stop).Source
            "=== $($runtime.Name): $executable ==="
            & $executable -e $runtime.Probe
            if ($LASTEXITCODE -ne 0) { throw "runtime identity probe exited $LASTEXITCODE" }
            & $executable scripts/run_busted.lua @BustedArgs
            $result = $LASTEXITCODE
            "$($runtime.Name) Busted exit: $result"
            if ($result -ne 0) { $failures++ }
        } catch {
            "$($runtime.Name) FAILED: $($_.Exception.Message)"
            $failures++
        }
    }
} finally {
    $env:LUA_PATH = $previousPath
    $env:LUA_CPATH = $previousCPath
    Pop-Location
}
if ($failures -gt 0) { exit 1 }
exit 0
