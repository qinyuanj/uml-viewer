param([string]$Love='C:/Program Files/LOVE/lovec.exe',[Parameter(Mandatory)][string]$CimguiDirectory,[Parameter(Mandatory)][string]$SarasaFont)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$evidence=Join-Path ([IO.Path]::GetTempPath()) ('uml-inspector-'+[guid]::NewGuid().ToString('N'))
$null=New-Item -ItemType Directory $evidence
$path=Join-Path $evidence 'diagram.edn'
[IO.File]::WriteAllText($path,'{:title "中文 Inspector" :hierarchical true :prefix "demo" :classes [{:id :a :name "中文类" :ns "demo.a" :ops [{:name "work" :text "work()"}]}] :proposals [{:id :plan :name "中文方案" :layers []}]}')
$names=@('UML_CIMGUI_DIRECTORY','UML_VIEWER_EVIDENCE','UML_VIEWER_TEST_SECONDS','UML_VIEWER_TEST_DRIVER','UML_VIEWER_PROJECT_ROOT','UML_CJK_FONT','UML_FONT_TEST_EXPECT'); $previous=@{}
foreach($name in $names){$previous[$name]=[Environment]::GetEnvironmentVariable($name)}
try {
  $env:UML_CIMGUI_DIRECTORY=(Resolve-Path $CimguiDirectory).Path
  # Provision Sarasa into the isolated project the way get-uml-viewer does.
  $null=New-Item -ItemType Directory "$evidence/.uml-viewer/fonts"; Copy-Item (Resolve-Path $SarasaFont).Path "$evidence/.uml-viewer/fonts/SarasaMonoSC-Regular.ttf"
  $env:UML_VIEWER_PROJECT_ROOT=$evidence; $env:UML_CJK_FONT=$null; $env:UML_FONT_TEST_EXPECT='sarasa'
  $env:UML_VIEWER_EVIDENCE=$evidence; $env:UML_VIEWER_TEST_SECONDS='30'; $env:UML_VIEWER_TEST_DRIVER=Join-Path $repo 'scripts/test_love_inspector.lua'
  $info=[Diagnostics.ProcessStartInfo]::new($Love); $info.UseShellExecute=$false; $info.WorkingDirectory=$repo; $info.RedirectStandardOutput=$true; $info.RedirectStandardError=$true
  foreach($a in @((Join-Path $repo 'love'),'viewer',$path)){$info.ArgumentList.Add($a)}
  $p=[Diagnostics.Process]::Start($info); $stdout=$p.StandardOutput.ReadToEndAsync(); $stderr=$p.StandardError.ReadToEndAsync()
  if(-not $p.WaitForExit(45000)){throw "Owned inspector test did not exit: pid $($p.Id), evidence $evidence"}
  $out=$stdout.GetAwaiter().GetResult(); $err=$stderr.GetAwaiter().GetResult()
  [IO.File]::WriteAllText("$evidence/stdout.txt",$out); [IO.File]::WriteAllText("$evidence/stderr.txt",$err)
  if($p.ExitCode -ne 0){throw "LOVE failed ($($p.ExitCode)): $err $out Evidence: $evidence"}
  foreach($file in @('inspector-pass.txt','inspector.png')){if(-not(Test-Path "$evidence/$file")){throw "Missing $file. Evidence: $evidence"}}
  "PASS native inspector Chinese proposal text input and P. Evidence: $evidence"
} finally {foreach($name in $names){[Environment]::SetEnvironmentVariable($name,$previous[$name])}}
