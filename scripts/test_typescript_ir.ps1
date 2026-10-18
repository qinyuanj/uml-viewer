param(
  [string]$Love = 'C:/Program Files/LOVE/lovec.exe',
  [string]$Lua54 = "$env:LOCALAPPDATA/Programs/Lua/bin/lua.exe"
)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$testRoot=Join-Path ([IO.Path]::GetTempPath()) ('uml58-cli-'+[guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory($testRoot) | Out-Null
$start=[Diagnostics.ProcessStartInfo]::new($Love)
$start.WorkingDirectory=$repo
$start.UseShellExecute=$false
$start.RedirectStandardOutput=$true
$start.RedirectStandardError=$true
$out=Join-Path $testRoot 'typescript.edn'
foreach($arg in @((Join-Path $repo 'love'),'ir','--root',(Join-Path $repo 'typescript-fixture'),'--policy','policy.edn','--output',$out)) { $start.ArgumentList.Add($arg) }
$process=[Diagnostics.Process]::Start($start)
$stdout=$process.StandardOutput.ReadToEndAsync()
$stderr=$process.StandardError.ReadToEndAsync()
if(-not $process.WaitForExit(15000)) { $null=$process.CloseMainWindow(); throw 'TypeScript IR exceeded 15 seconds; requested normal window close' }
$text=$stdout.GetAwaiter().GetResult(); $err=$stderr.GetAwaiter().GetResult()
if($process.ExitCode -ne 0 -or $err) { throw "TypeScript CLI exit=$($process.ExitCode): $text $err" }
& $Lua54 (Join-Path $repo 'scripts/compare_ir.lua') $out (Join-Path $repo 'fixtures/jvm-goldens/typescript.ir.edn')
if($LASTEXITCODE -ne 0) { throw 'TypeScript JVM golden semantic comparison failed' }
"Public headless LOVE TypeScript CLI exit=0; golden semantic equality passed; artifact: $out"
