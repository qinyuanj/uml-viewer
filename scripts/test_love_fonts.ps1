# Native font lookup chain: provisioned Sarasa, UML_CJK_FONT override and the
# YaHei fallback warning, each in an isolated temporary project.
param([string]$Love='C:/Program Files/LOVE/lovec.exe',[Parameter(Mandatory)][string]$CimguiDirectory,[Parameter(Mandatory)][string]$SarasaFont)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$sarasa=(Resolve-Path $SarasaFont).Path
$names=@('UML_CIMGUI_DIRECTORY','UML_VIEWER_EVIDENCE','UML_VIEWER_PROJECT_ROOT','UML_VIEWER_TEST_SECONDS','UML_VIEWER_TEST_DRIVER','UML_CJK_FONT','UML_FONT_TEST_EXPECT','UML_FONT_TEST_EXTRA'); $previous=@{}
foreach($name in $names){$previous[$name]=[Environment]::GetEnvironmentVariable($name)}
function Run-Case([string]$Case,[string]$Expect,[bool]$Provision,[string]$Override){
  $project=Join-Path ([IO.Path]::GetTempPath()) ("uml-fonts-$Case-"+[guid]::NewGuid().ToString('N'))
  $null=New-Item -ItemType Directory $project
  if($Provision){ $null=New-Item -ItemType Directory "$project/.uml-viewer/fonts"; Copy-Item $sarasa "$project/.uml-viewer/fonts/SarasaMonoSC-Regular.ttf" }
  $path=Join-Path $project 'diagram.edn'
  [IO.File]::WriteAllText($path,'{:hierarchical true :prefix "demo" :classes [{:id :a :name "中文类名" :ns "demo.a"}] :proposals [{:id :plan :name "中文提案" :layers []}]}')
  $env:UML_CIMGUI_DIRECTORY=(Resolve-Path $CimguiDirectory).Path
  $env:UML_VIEWER_EVIDENCE=$project; $env:UML_VIEWER_PROJECT_ROOT=$project; $env:UML_VIEWER_TEST_SECONDS='30'
  $env:UML_VIEWER_TEST_DRIVER=Join-Path $repo 'scripts/test_love_fonts.lua'
  $env:UML_CJK_FONT=$Override; $env:UML_FONT_TEST_EXPECT=$Expect; $env:UML_FONT_TEST_EXTRA='中文类名提案'
  $info=[Diagnostics.ProcessStartInfo]::new($Love); $info.UseShellExecute=$false; $info.WorkingDirectory=$project; $info.RedirectStandardOutput=$true; $info.RedirectStandardError=$true
  foreach($a in @((Join-Path $repo 'love'),'viewer',$path)){$info.ArgumentList.Add($a)}
  $p=[Diagnostics.Process]::Start($info); $stdout=$p.StandardOutput.ReadToEndAsync(); $stderr=$p.StandardError.ReadToEndAsync()
  if(-not $p.WaitForExit(45000)){throw "Owned font test $Case did not exit: pid $($p.Id), evidence $project"}
  $err=$stderr.GetAwaiter().GetResult(); [IO.File]::WriteAllText("$project/stderr.txt",$err); [IO.File]::WriteAllText("$project/stdout.txt",$stdout.GetAwaiter().GetResult())
  if($p.ExitCode -ne 0 -or -not (Test-Path "$project/fonts-pass.txt")){throw "Font case $Case failed ($($p.ExitCode)): $err Evidence: $project"}
  $warnings=([regex]::Matches($err,'falling back to Microsoft YaHei')).Count
  $expected=if($Expect -eq 'yahei'){1}else{0}
  if($warnings -ne $expected){throw "Font case $Case printed $warnings YaHei warnings, expected $expected. Evidence: $project"}
  if($Expect -eq 'yahei' -and $err -notmatch 'get-uml-viewer'){throw "YaHei warning does not mention get-uml-viewer. Evidence: $project"}
  if($Provision){ Remove-Item -LiteralPath "$project/.uml-viewer/fonts" -Recurse -Force }
  "PASS $Case $((Get-Content "$project/fonts-pass.txt" -Raw).Trim()) Evidence: $project"
}
try {
  Run-Case 'provisioned' 'sarasa' $true ''
  Run-Case 'override' 'env' $false $sarasa
  Run-Case 'fallback' 'yahei' $false ''
} finally {foreach($name in $names){[Environment]::SetEnvironmentVariable($name,$previous[$name])}}
