param(
  [string]$Love='C:/Program Files/LOVE/lovec.exe',
  [Parameter(Mandatory)][string]$CimguiDirectory
)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$evidence=Join-Path ([IO.Path]::GetTempPath()) ('uml-68-jump-'+[guid]::NewGuid().ToString('N'))
$null=New-Item -ItemType Directory $evidence
$source=Join-Path $evidence 'source.lua'
$diagram=Join-Path $evidence 'diagram.edn'
[IO.File]::WriteAllText($source,"local M = {}`n"+((1..80 | ForEach-Object { "-- preceding line $_`n" }) -join '')+"function M.target() return 'target ## %' end`nreturn M`n")
$sourceEDN=$source.Replace('\','/')
[IO.File]::WriteAllText($diagram,('{:title "Source jump acceptance" :classes [{:id :app.some_module :name "some_module" :ns "app.some_module" :lang :lua :file "'+$sourceEDN+'" :ops [{:name "M.target"}]}] :edges []}'))
$names=@('UML_CIMGUI_DIRECTORY','UML_VIEWER_TEST_SECONDS','UML_VIEWER_TEST_DRIVER','UML68_EVIDENCE')
$previous=@{}
foreach ($name in $names) { $previous[$name]=[Environment]::GetEnvironmentVariable($name) }
try {
  $env:UML_CIMGUI_DIRECTORY=(Resolve-Path $CimguiDirectory).Path
  $env:UML_VIEWER_TEST_SECONDS='12'
  $env:UML_VIEWER_TEST_DRIVER=Join-Path $repo 'scripts/test_love_source_jump.lua'
  $env:UML68_EVIDENCE=$evidence
  $info=[Diagnostics.ProcessStartInfo]::new($Love)
  $info.UseShellExecute=$false; $info.WorkingDirectory=$repo
  $info.RedirectStandardOutput=$true; $info.RedirectStandardError=$true
  foreach ($a in @((Join-Path $repo 'love'),'viewer',$diagram)) { $info.ArgumentList.Add($a) }
  $process=[Diagnostics.Process]::Start($info)
  $stdout=$process.StandardOutput.ReadToEndAsync(); $stderr=$process.StandardError.ReadToEndAsync()
  if (-not $process.WaitForExit(20000)) { throw "Owned jump process did not exit: $($process.Id); $evidence" }
  $out=$stdout.GetAwaiter().GetResult(); $err=$stderr.GetAwaiter().GetResult()
  [IO.File]::WriteAllText("$evidence/stdout.txt",$out); [IO.File]::WriteAllText("$evidence/stderr.txt",$err)
  if ($process.ExitCode -ne 0) { throw "Source jump failed ($($process.ExitCode)): $err $out; $evidence" }
  foreach ($file in @('module.png','module.txt','member.png','member.txt','source-jump-pass.txt')) {
    if (-not (Test-Path "$evidence/$file")) { throw "Missing $file; $evidence" }
  }
  "PASS native selected-class/module and member source jumps; file $source line82. Evidence: $evidence"
} finally {
  foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name,$previous[$name]) }
}
