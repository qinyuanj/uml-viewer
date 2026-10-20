param(
  [string]$Love='C:/Program Files/LOVE/lovec.exe',
  [Parameter(Mandatory)][string]$CimguiDirectory,
  [ValidateSet('clipboard','throw')][string]$Case='clipboard'
)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$evidence=Join-Path ([IO.Path]::GetTempPath()) ('uml-78-review-'+$Case+'-'+[guid]::NewGuid().ToString('N'))
$null=New-Item -ItemType Directory $evidence
$path=Join-Path $evidence 'diagram.edn'
[IO.File]::WriteAllText($path,'{:packages [{:id :p :classes [{:id :a}]}]}')
$names=@('UML_CIMGUI_DIRECTORY','UML_VIEWER_TEST_SECONDS','UML_VIEWER_TEST_DRIVER','UML_VIEWER_PROJECT_ROOT','UML78_EVIDENCE','UML78_THROW')
$previous=@{}
foreach ($name in $names) { $previous[$name]=[Environment]::GetEnvironmentVariable($name) }
try {
  $env:UML_CIMGUI_DIRECTORY=(Resolve-Path $CimguiDirectory).Path
  $env:UML_VIEWER_TEST_SECONDS='30'
  $env:UML_VIEWER_TEST_DRIVER=Join-Path $repo 'scripts/test_love_review_fixes.lua'
  $env:UML_VIEWER_PROJECT_ROOT=$evidence
  $env:UML78_EVIDENCE=$evidence
  $env:UML78_THROW=($Case -eq 'throw') ? '1' : '0'
  $info=[Diagnostics.ProcessStartInfo]::new($Love)
  $info.UseShellExecute=$false; $info.WorkingDirectory=$repo
  $info.RedirectStandardOutput=$true; $info.RedirectStandardError=$true
  foreach ($a in @((Join-Path $repo 'love'),'viewer',$path)) { $info.ArgumentList.Add($a) }
  $process=[Diagnostics.Process]::Start($info)
  $stdout=$process.StandardOutput.ReadToEndAsync(); $stderr=$process.StandardError.ReadToEndAsync()
  if (-not $process.WaitForExit(40000)) { throw "Owned review process did not exit: $($process.Id); $evidence" }
  $out=$stdout.GetAwaiter().GetResult(); $err=$stderr.GetAwaiter().GetResult()
  [IO.File]::WriteAllText("$evidence/stdout.txt",$out); [IO.File]::WriteAllText("$evidence/stderr.txt",$err)
  [IO.File]::WriteAllText("$evidence/exit.txt",[string]$process.ExitCode)
  if ($Case -eq 'throw') {
    if ($process.ExitCode -ne 1 -or $err -notmatch 'Navigation acceptance failed: .*intentional first-frame probe failure') { throw "Expected exit 1 and original stderr; actual $($process.ExitCode): $err; $evidence" }
  } elseif ($process.ExitCode -ne 0 -or -not (Test-Path "$evidence/clipboard-pass.txt")) { throw "Clipboard regression failed ($($process.ExitCode)): $err $out; $evidence" }
  "PASS native $Case; exit $($process.ExitCode); evidence $evidence"
} finally {
  foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name,$previous[$name]) }
}
