param(
  [string]$Love = 'C:/Program Files/LOVE/lovec.exe',
  [string]$Lua54 = "$env:LOCALAPPDATA/Programs/Lua/bin/lua.exe"
)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$testRoot=Join-Path ([IO.Path]::GetTempPath()) ('uml60-python-'+[guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory($testRoot) | Out-Null
# Extract the immutable LF fixture used by the independent JVM oracle. The native
# checkout may use CRLF; Python's original exact backslash-LF rule is preserved.
$archive=Join-Path $testRoot 'fixture.tar'
& git -C $repo archive --format=tar "--output=$archive" HEAD python-fixture
if($LASTEXITCODE -ne 0) { throw 'Could not archive the committed Python fixture' }
& tar -xf $archive -C $testRoot
if($LASTEXITCODE -ne 0) { throw 'Could not stage the committed Python fixture' }
$fixture=Join-Path $testRoot 'python-fixture'
function Invoke-PythonIr([string]$Policy,[string]$Output) {
  $start=[Diagnostics.ProcessStartInfo]::new($Love)
  $start.WorkingDirectory=$repo
  $start.UseShellExecute=$false
  $start.RedirectStandardOutput=$true
  $start.RedirectStandardError=$true
  foreach($a in @((Join-Path $repo 'love'),'ir','--root',$fixture,'--policy',$Policy,'--output',$Output)) { $start.ArgumentList.Add($a) }
  $process=[Diagnostics.Process]::Start($start)
  $stdout=$process.StandardOutput.ReadToEndAsync(); $stderr=$process.StandardError.ReadToEndAsync()
  if(-not $process.WaitForExit(10000)) { $null=$process.CloseMainWindow(); throw 'Python IR exceeded 10 seconds; requested normal close' }
  $out=$stdout.GetAwaiter().GetResult(); $err=$stderr.GetAwaiter().GetResult()
  if($process.ExitCode -ne 0 -or $err) { throw "Python IR failed exit=$($process.ExitCode): $out $err" }
  & $Lua54 (Join-Path $repo 'scripts/compare_ir.lua') $Output (Join-Path $repo 'fixtures/jvm-goldens/python.ir.edn')
  if($LASTEXITCODE -ne 0) { throw 'Python IR differs from the immutable JVM golden' }
}
Invoke-PythonIr 'policy.edn' (Join-Path $testRoot 'explicit.edn')
# Keep the same policy-file value in the public result while exercising #61's
# pyproject.toml detection in a second invocation.
$policyPath=Join-Path $fixture 'policy.edn'
$policy=[IO.File]::ReadAllText($policyPath).Replace(':lang :python','')
[IO.File]::WriteAllText($policyPath,$policy,[Text.UTF8Encoding]::new($false))
Invoke-PythonIr 'policy.edn' (Join-Path $testRoot 'detected.edn')
"Python explicit and detected public LOVE CLI golden checks passed; artifacts: $testRoot"
