# Native Windows regression: real pwsh -File uml.ps1 -> Clojure CLI -> JVM.
# Requires PowerShell 7 and Java; missing CLI/module prerequisites report pending.
$ErrorActionPreference = 'Stop'
if (-not $IsWindows -or $PSVersionTable.PSVersion.Major -lt 7) {
    throw 'Run this test with PowerShell 7 on native Windows.'
}
$nativeCli = Get-Command clojure.exe -CommandType Application -ErrorAction SilentlyContinue
$clojureTools = $null
$moduleReason = $null
try { $clojureTools = Import-Module ClojureTools -PassThru -ErrorAction Stop }
catch { $moduleReason = $_.Exception.Message }
function Native-Path([string]$Path) {
    return [IO.Path]::GetFullPath(([uri]$Path).LocalPath)
}
$repoScripts = Native-Path $PSScriptRoot
$temp = Native-Path (Join-Path $env:LOCALAPPDATA ('Temp/uml native entries ' + [guid]::NewGuid().ToString('N')))
$project = Native-Path (Join-Path $temp 'project with spaces')
$installed = Join-Path $project '.uml-viewer/uml-viewer/scripts'
$originalPath = $env:PATH
$passed = 0
$pending = 0
function Assert-Equal($Expected, $Actual, [string]$Message) {
    if ($Expected -cne $Actual) { throw "$Message`nExpected: $Expected`nActual: $Actual" }
}
function Run-Entry([string]$Alias, [switch]$WithoutModules, [switch]$WithModule) {
    # Start-Process re-parses ArgumentList; spaced paths must include quotes.
    $entry = if ($WithoutModules) { Join-Path $temp 'without modules.ps1' }
             elseif ($WithModule) { Join-Path $temp 'with module.ps1' }
             else { Join-Path $project 'uml.ps1' }
    $process = Start-Process -FilePath 'pwsh.exe' `
        -ArgumentList @('-NoLogo', '-NoProfile', '-File', ('"' + $entry + '"'),
                        $Alias, '"src/file with spaces.clj"', '"literal;$value"') `
        -WorkingDirectory $temp `
        -RedirectStandardOutput (Join-Path $temp 'stdout.txt') -RedirectStandardError (Join-Path $temp 'stderr.txt') -Wait -PassThru -NoNewWindow
    $result = @{ Code = $process.ExitCode; Out = Get-Content (Join-Path $temp 'stdout.txt') -Raw; Err = Get-Content (Join-Path $temp 'stderr.txt') -Raw }
    $process.Dispose()
    return $result
}
function Assert-Result($Result, [string[]]$Argv) {
    Assert-Equal 23 $Result.Code ("uml.ps1 must retain alias exit status. stdout=$($Result.Out) stderr=$($Result.Err)")
    $raw = [IO.File]::ReadAllBytes((Join-Path $temp 'stdout.txt'))
    $ansi = [regex]'\x1b\[[0-9;]*m'
    $lines = @($ansi.Replace([Text.Encoding]::UTF8.GetString($raw), '') -split '\r?\n' | Where-Object {
        $_ -and $_ -notmatch '^WARNING: Implicit use of clojure.main' -and
        $_ -notmatch '^\s+目录:' -and $_ -notmatch '^\s*Mode\s+LastWriteTime' -and
        $_ -notmatch '^[-\s]*$' -and $_ -notmatch '^d[-a-z]+\s' -and $_ -notmatch '^-a[-a-z]+\s' -and
        $_ -notmatch '^\s+警告:' -and $_ -notmatch '^\s+Mode\s+$' -and $_ -notmatch '^\s+----\s+' -and
        $_ -notmatch '^\s*Length\s+Name\s*$' -and $_ -notmatch '^\s+LastWriteTime\s+$'
    })
    Assert-Equal 'fixture-stdout' $lines[0] 'Alias stdout must reach the caller.'
    Assert-Equal $project ([Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($lines[1]))) 'Alias cwd must be the project.'
    $received = @($lines | Select-Object -Skip 2 | ForEach-Object { [Text.Encoding]::UTF8.GetString([Convert]::FromBase64String($_)) })
    Assert-Equal ($Argv | ConvertTo-Json -Compress) ($received | ConvertTo-Json -Compress) 'Alias arguments must remain separate and literal.'
    Assert-Equal 'fixture-stderr' $Result.Err.Trim() 'Alias stderr must reach the caller.'
}
try {
    $null = New-Item -ItemType Directory -Path $installed -Force
    Copy-Item (Join-Path $repoScripts 'uml.ps1') $project
    foreach ($script in @('uml-command.ps1', 'uml-quality-config.ps1', 'uml-standard-quality.ps1')) {
        Copy-Item (Join-Path $repoScripts $script) $installed
    }
    @'
{:paths ["src"]
 :aliases {:crap {:main-opts ["-m" "entry-fixture" "crap"]}
           :mutate {:main-opts ["-m" "entry-fixture" "mutate"]}}}
'@ | Set-Content (Join-Path $project 'deps.edn') -Encoding utf8
    $null = New-Item -ItemType Directory -Path (Join-Path $project 'src')
    @'
(ns entry-fixture)
(defn encoded [s] (.encodeToString (java.util.Base64/getEncoder) (.getBytes s "UTF-8")))
(defn -main [& args]
  (.println System/out "fixture-stdout")
  (.println System/out (encoded (System/getProperty "user.dir")))
  (doseq [arg args] (.println System/out (encoded arg)))
  (.flush System/out)
  (.println System/err "fixture-stderr")
  (.flush System/err)
  (System/exit 23))
'@ | Set-Content (Join-Path $project 'src/entry_fixture.clj') -Encoding utf8
    # The executable consumes -M:<alias>; main-opts supplies the alias to -main.
    if ($nativeCli) { $env:PATH = (Split-Path $nativeCli.Source) + ';' + $originalPath }
    foreach ($alias in @('crap', 'mutate')) {
        if (-not $nativeCli) {
            Write-Host "PENDING clojure.exe uml.ps1 ${alias}: clojure.exe is not on PATH"
            $pending++
            continue
        }
        Assert-Result (Run-Entry $alias) @($alias, 'src/file with spaces.clj', 'literal;$value')
        Write-Host "PASS clojure.exe uml.ps1 ${alias}: real JVM argv, cwd, streams, exit=23"
        $passed++
    }
    # Import inside the child so the module command wins over any installed exe.
    @'
Import-Module ClojureTools -ErrorAction Stop
& (Join-Path $PSScriptRoot 'project with spaces/uml.ps1') @args
exit $LASTEXITCODE
'@ | Set-Content (Join-Path $temp 'with module.ps1') -Encoding utf8
    foreach ($alias in @('crap', 'mutate')) {
        if (-not $clojureTools) {
            Write-Host "PENDING ClojureTools uml.ps1 ${alias}: $moduleReason"
            $pending++
            continue
        }
        Assert-Result (Run-Entry $alias -WithModule) @($alias, 'src/file with spaces.clj', 'literal;$value')
        Write-Host "PASS ClojureTools uml.ps1 ${alias}: argv, cwd, streams, exit=23"
        $passed++
    }
    $tools = Join-Path $temp 'external tools'
    $null = New-Item -ItemType Directory -Path $tools
    @'
Write-Output 'fixture-stdout'
@((Get-Location).Path) + @($args) | ForEach-Object {
    [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($_))
}
[Console]::Error.WriteLine('fixture-stderr')
exit 23
'@ | Set-Content (Join-Path $tools 'clojure.ps1') -Encoding utf8
    # A PATH PS1 makes uml-command.ps1 import the module even when an exe exists.
    $env:PATH = $tools + ';' + $originalPath
    foreach ($alias in @('crap', 'mutate')) {
        if (-not $clojureTools) {
            Write-Host "PENDING PATH-shim uml.ps1 ${alias}: ClojureTools module required to resolve over shim; $moduleReason"
            $pending++
            continue
        }
        Assert-Result (Run-Entry $alias) @($alias, 'src/file with spaces.clj', 'literal;$value')
        Write-Host "PASS PATH-shim uml.ps1 ${alias}: module alias resolved over shim, exit=23"
        $passed++
    }
    # PS7 restores default module paths at startup; clear them inside the child.
    $null = New-Item -ItemType Directory -Path (Join-Path $temp 'empty modules')
    @'
$env:PSModulePath = Join-Path $PSScriptRoot 'empty modules'
& (Join-Path $PSScriptRoot 'project with spaces/uml.ps1') @args
exit $LASTEXITCODE
'@ | Set-Content (Join-Path $temp 'without modules.ps1') -Encoding utf8
    foreach ($alias in @('crap', 'mutate')) {
        Assert-Result (Run-Entry $alias -WithoutModules) @("-M:$alias", 'src/file with spaces.clj', 'literal;$value')
        Write-Host "PASS ExternalScript uml.ps1 ${alias}: original argv, cwd, streams, exit=23"
        $passed++
    }
    Write-Host "passed=$passed pending=$pending"
} finally {
    $env:PATH = $originalPath
    Remove-Item -LiteralPath $temp -Recurse -Force
}
