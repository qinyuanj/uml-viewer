param([string]$Love='C:/Program Files/LOVE/lovec.exe',[Parameter(Mandatory)][string]$CimguiDirectory)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$evidence=Join-Path ([IO.Path]::GetTempPath()) ('uml-class-boxes-'+[guid]::NewGuid().ToString('N'))
$null=New-Item -ItemType Directory $evidence
$path=Join-Path $evidence 'diagram.edn'
[IO.File]::WriteAllText($path,'{:title "Classifier parity" :packages [{:label "Pooled quality" :classes [{:id :abstract :name "Abstract" :stereotype :abstract :level 0 :crap {:mu 2} :killed 1 :survived 0 :ops ["work()"]} {:id :interface :name "Interface" :stereotype :interface :crap {:mu 2} :ops ["execute()"]}]}]}')
$names=@('UML_CIMGUI_DIRECTORY','UML_VIEWER_EVIDENCE','UML_VIEWER_TEST_SECONDS','UML_VIEWER_TEST_DRIVER'); $previous=@{}
foreach($name in $names){$previous[$name]=[Environment]::GetEnvironmentVariable($name)}
try {
  $env:UML_CIMGUI_DIRECTORY=(Resolve-Path $CimguiDirectory).Path
  $env:UML_VIEWER_EVIDENCE=$evidence; $env:UML_VIEWER_TEST_SECONDS='3'; $env:UML_VIEWER_TEST_DRIVER=Join-Path $repo 'scripts/test_love_class_boxes.lua'
  $info=[Diagnostics.ProcessStartInfo]::new($Love); $info.UseShellExecute=$false; $info.WorkingDirectory=$repo; $info.RedirectStandardOutput=$true; $info.RedirectStandardError=$true
  foreach($a in @((Join-Path $repo 'love'),'viewer',$path)){$info.ArgumentList.Add($a)}
  $p=[Diagnostics.Process]::Start($info); $stdout=$p.StandardOutput.ReadToEndAsync(); $stderr=$p.StandardError.ReadToEndAsync()
  if(-not $p.WaitForExit(15000)){throw "Owned smoke did not exit: pid $($p.Id), evidence $evidence"}
  $out=$stdout.GetAwaiter().GetResult(); $err=$stderr.GetAwaiter().GetResult()
  [IO.File]::WriteAllText("$evidence/stdout.txt",$out); [IO.File]::WriteAllText("$evidence/stderr.txt",$err)
  if($p.ExitCode -ne 0){throw "LOVE failed ($($p.ExitCode)): $err $out Evidence: $evidence"}
  foreach($file in @('class-boxes-pass.txt','render.png')){if(-not(Test-Path "$evidence/$file")){throw "Missing $file. Evidence: $evidence"}}
  "PASS native classifier/package render and glyph style. Evidence: $evidence"
} finally {foreach($name in $names){[Environment]::SetEnvironmentVariable($name,$previous[$name])}}
