param([string]$Love='C:/Program Files/LOVE/lovec.exe',[Parameter(Mandatory)][string]$CimguiDirectory)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$evidence=Join-Path ([IO.Path]::GetTempPath()) ('uml-detail-scroll-'+[guid]::NewGuid().ToString('N'))
$null=New-Item -ItemType Directory $evidence
$ops=(1..40 | ForEach-Object { '{:name "op' + $_ + '" :text "op' + $_ + '()"}' }) -join ' '
$path=Join-Path $evidence 'diagram.edn'
[IO.File]::WriteAllText($path,'{:title "Scroll Inspector" :hierarchical true :prefix "demo" :classes [{:id :a :name "A" :ns "demo.a" :ops ['+$ops+']}] :proposals [{:id :plan :name "Plan A" :layers []}]}')
$names=@('UML_CIMGUI_DIRECTORY','UML_VIEWER_EVIDENCE','UML_VIEWER_PROJECT_ROOT','UML_VIEWER_TEST_SECONDS','UML_VIEWER_TEST_DRIVER'); $previous=@{}
foreach($name in $names){$previous[$name]=[Environment]::GetEnvironmentVariable($name)}
try {
  $env:UML_CIMGUI_DIRECTORY=(Resolve-Path $CimguiDirectory).Path
  $env:UML_VIEWER_EVIDENCE=$evidence; $env:UML_VIEWER_PROJECT_ROOT=$evidence
  $env:UML_VIEWER_TEST_SECONDS='30'; $env:UML_VIEWER_TEST_DRIVER=Join-Path $repo 'scripts/test_love_detail_scroll.lua'
  $info=[Diagnostics.ProcessStartInfo]::new($Love); $info.UseShellExecute=$false; $info.WorkingDirectory=$repo; $info.RedirectStandardOutput=$true; $info.RedirectStandardError=$true
  foreach($a in @((Join-Path $repo 'love'),'viewer',$path)){$info.ArgumentList.Add($a)}
  $p=[Diagnostics.Process]::Start($info); $stdout=$p.StandardOutput.ReadToEndAsync(); $stderr=$p.StandardError.ReadToEndAsync()
  if(-not $p.WaitForExit(45000)){throw "Owned detail-scroll test did not exit: pid $($p.Id), evidence $evidence"}
  $out=$stdout.GetAwaiter().GetResult(); $err=$stderr.GetAwaiter().GetResult()
  [IO.File]::WriteAllText("$evidence/stdout.txt",$out); [IO.File]::WriteAllText("$evidence/stderr.txt",$err)
  if($p.ExitCode -ne 0){throw "LOVE failed ($($p.ExitCode)): $err $out Evidence: $evidence"}
  foreach($file in @('inspector-pass.txt','inspector.png','inspector-empty.png')){if(-not(Test-Path "$evidence/$file")){throw "Missing $file. Evidence: $evidence"}}
  "PASS native detail scroll, empty-inspector copy, hovered op and proposal controls. Evidence: $evidence"
} finally {foreach($name in $names){[Environment]::SetEnvironmentVariable($name,$previous[$name])}}
