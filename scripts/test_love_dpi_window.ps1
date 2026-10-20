param([string]$Love='C:/Program Files/LOVE/lovec.exe',[Parameter(Mandatory)][string]$CimguiDirectory,[Parameter(Mandatory)][string]$SarasaFont,[double[]]$Scales=@(1,1.25,1.5))
# Isolated #84 acceptance: owned LOVE viewers in a temporary project root at
# simulated display scales. Never touches a running viewer or this repository's
# .uml-viewer state.
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$evidence=Join-Path ([IO.Path]::GetTempPath()) ('uml-dpi-window-'+[guid]::NewGuid().ToString('N'))
$null=New-Item -ItemType Directory $evidence
$names=@('UML_CIMGUI_DIRECTORY','UML_VIEWER_EVIDENCE','UML_VIEWER_TEST_SECONDS','UML_VIEWER_TEST_DRIVER','UML_VIEWER_PROJECT_ROOT','UML_CJK_FONT','UML_VIEWER_TEST_DPI_SCALE','UML_DPI_TEST_PHASE','UML_DPI_TEST_TAG'); $previous=@{}
foreach($name in $names){$previous[$name]=[Environment]::GetEnvironmentVariable($name)}
function Invoke-Viewer([string]$Root,[double]$Scale,[string]$Phase,[string]$Tag,[string[]]$Extra){
  $env:UML_VIEWER_PROJECT_ROOT=$Root; $env:UML_VIEWER_TEST_DPI_SCALE=[string]$Scale; $env:UML_DPI_TEST_PHASE=$Phase; $env:UML_DPI_TEST_TAG=$Tag
  $info=[Diagnostics.ProcessStartInfo]::new($Love); $info.UseShellExecute=$false; $info.WorkingDirectory=$repo; $info.RedirectStandardOutput=$true; $info.RedirectStandardError=$true
  foreach($a in @((Join-Path $repo 'love'),'viewer')+$Extra){$info.ArgumentList.Add($a)}
  $p=[Diagnostics.Process]::Start($info); $stdout=$p.StandardOutput.ReadToEndAsync(); $stderr=$p.StandardError.ReadToEndAsync()
  if(-not $p.WaitForExit(45000)){throw "Owned DPI viewer did not exit: pid $($p.Id), evidence $evidence"}
  $out=$stdout.GetAwaiter().GetResult(); $err=$stderr.GetAwaiter().GetResult()
  [IO.File]::WriteAllText("$evidence/$Tag-stdout.txt",$out); [IO.File]::WriteAllText("$evidence/$Tag-stderr.txt",$err)
  if($p.ExitCode -ne 0){throw "LOVE $Tag failed ($($p.ExitCode)): $err $out Evidence: $evidence"}
  if($err -match 'failed|rejected'){throw "LOVE $Tag reported: $err Evidence: $evidence"}
  if(-not(Test-Path "$evidence/$Tag-pass.edn")){throw "Missing $Tag-pass.edn. Evidence: $evidence"}
}
function New-Project([string]$Name){
  $root=Join-Path $evidence $Name
  $null=New-Item -ItemType Directory "$root/.uml-viewer/fonts"
  Copy-Item (Resolve-Path $SarasaFont).Path "$root/.uml-viewer/fonts/SarasaMonoSC-Regular.ttf"
  [IO.File]::WriteAllText("$root/diagram.edn",'{:title "DPI 角标" :hierarchical true :prefix "demo" :classes [{:id :a :name "中文类" :ns "demo.a" :level 2 :stereotype :interface :ops [{:name "work" :text "work()"}]} {:id :b :name "Beta" :ns "demo.b" :stereotype :abstract :fields ["count"]}] :edges [{:from :a :to :b :kind :dependency}]}')
  return $root
}
try {
  $env:UML_CIMGUI_DIRECTORY=(Resolve-Path $CimguiDirectory).Path
  $env:UML_CJK_FONT=$null; $env:UML_VIEWER_EVIDENCE=$evidence; $env:UML_VIEWER_TEST_SECONDS='30'
  $env:UML_VIEWER_TEST_DRIVER=Join-Path $repo 'scripts/test_love_dpi_window.lua'
  $results=@()
  foreach($scale in $Scales){
    $label=([string]$scale).Replace('.','_')
    $root=New-Project "dpi-$label"
    Invoke-Viewer $root $scale 'first' "first-$label" @("$root/diagram.edn")
    if(-not(Test-Path "$root/.uml-viewer/session.edn")){throw "quit-for-restart saved no session at $scale. Evidence: $evidence"}
    Invoke-Viewer $root $scale 'restart' "restart-$label" @('--restart')
    $results+="dpi=$scale"
  }
  # A session saved before #84 has no window fields.
  $legacy=New-Project 'legacy'
  [IO.File]::WriteAllText("$legacy/.uml-viewer/session.edn","{:path `"$($legacy.Replace('\','/'))/diagram.edn`" :mail-seen 0 :focus [] :cam-x 7 :cam-y 0 :zoom 1.25}")
  Invoke-Viewer $legacy 1.25 'legacy' 'legacy' @('--restart')
  "PASS native DPI window, inspector drag, display, resize and restart restore ($($results -join ', '), legacy defaults). Evidence: $evidence"
} finally {foreach($name in $names){[Environment]::SetEnvironmentVariable($name,$previous[$name])}}
