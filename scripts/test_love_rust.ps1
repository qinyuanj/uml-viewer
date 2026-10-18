param(
  [string]$Love = 'C:/Program Files/LOVE/lovec.exe',
  [string]$Lua54 = "$env:LOCALAPPDATA/Programs/Lua/bin/lua.exe"
)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$testRoot=Join-Path ([IO.Path]::GetTempPath()) ('uml59-cli-'+[guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory($testRoot) | Out-Null
$output=Join-Path $testRoot 'rust.edn'
$start=[Diagnostics.ProcessStartInfo]::new($Love)
$start.WorkingDirectory=$repo
$start.UseShellExecute=$false
$start.RedirectStandardOutput=$true
$start.RedirectStandardError=$true
foreach($a in @((Join-Path $repo 'love'),'ir','--root',(Join-Path $repo 'rust-fixture'),'--policy','policy.edn','--output',$output)) { $start.ArgumentList.Add($a) }
$process=[Diagnostics.Process]::Start($start)
$stdout=$process.StandardOutput.ReadToEndAsync()
$stderr=$process.StandardError.ReadToEndAsync()
if(-not $process.WaitForExit(10000)) {
  $null=$process.CloseMainWindow()
  throw 'Rust IR exceeded 10 seconds; requested normal window close'
}
$out=$stdout.GetAwaiter().GetResult(); $err=$stderr.GetAwaiter().GetResult()
if($process.ExitCode -ne 0) { throw "Rust public CLI exit $($process.ExitCode): $out $err" }
if($err) { throw "Rust public CLI unexpected stderr: $err" }
& $Lua54 (Join-Path $repo 'scripts/compare_ir.lua') $output (Join-Path $repo 'fixtures/jvm-goldens/rust.ir.edn')
if($LASTEXITCODE -ne 0) { throw 'Rust JVM golden semantic comparison failed' }
"Rust headless LOVE public CLI passed; artifacts: $testRoot"
