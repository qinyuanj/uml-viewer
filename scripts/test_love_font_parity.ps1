param([string]$Love='C:/Program Files/LOVE/lovec.exe',[Parameter(Mandatory)][string]$CimguiDirectory,[Parameter(Mandatory)][string]$SarasaFont)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$names=@('UML_CIMGUI_DIRECTORY','UML_CJK_FONT','UML_VIEWER_EVIDENCE','UML_VIEWER_PROJECT_ROOT','UML_VIEWER_TEST_SECONDS','UML_VIEWER_TEST_DRIVER','UML_PARITY_SCENE'); $previous=@{}
foreach($name in $names){$previous[$name]=[Environment]::GetEnvironmentVariable($name)}
try {
  foreach($scene in @('baseline','chinese')) {
    $project=Join-Path ([IO.Path]::GetTempPath()) ('uml-font-parity-'+$scene+'-'+[guid]::NewGuid().ToString('N'))
    $null=New-Item -ItemType Directory $project
    $path=Join-Path $project 'diagram.edn'
    if($scene -eq 'baseline') {Copy-Item "$repo/examples/uml-viewer.edn" $path}
    else {[IO.File]::WriteAllText($path,'{:hierarchical true :prefix "demo" :classes [{:id :demo.detail :name "中文详情类名" :ns "demo.detail" :level 2 :stereotype :abstract :fields ["中文字段"] :ops [{:name "读取" :text "读取中文数据()" :crap 2 :coverage 0.9 :killed 3 :survived 0}]}]}')}
    $env:UML_CIMGUI_DIRECTORY=(Resolve-Path $CimguiDirectory).Path; $env:UML_CJK_FONT=(Resolve-Path $SarasaFont).Path
    $env:UML_VIEWER_EVIDENCE=$project; $env:UML_VIEWER_PROJECT_ROOT=$project; $env:UML_VIEWER_TEST_SECONDS='30'
    $env:UML_VIEWER_TEST_DRIVER=Join-Path $repo 'scripts/test_love_font_parity.lua'; $env:UML_PARITY_SCENE=$scene
    $info=[Diagnostics.ProcessStartInfo]::new($Love); $info.UseShellExecute=$false; $info.WorkingDirectory=$project; $info.RedirectStandardOutput=$true; $info.RedirectStandardError=$true
    foreach($a in @((Join-Path $repo 'love'),'viewer',$path)){$info.ArgumentList.Add($a)}
    $p=[Diagnostics.Process]::Start($info); $stdout=$p.StandardOutput.ReadToEndAsync(); $stderr=$p.StandardError.ReadToEndAsync()
    if(-not $p.WaitForExit(45000)){throw "Owned parity test did not exit: pid $($p.Id), evidence $project"}
    $err=$stderr.GetAwaiter().GetResult(); [IO.File]::WriteAllText("$project/stderr.txt",$err); [IO.File]::WriteAllText("$project/stdout.txt",$stdout.GetAwaiter().GetResult())
    if($p.ExitCode -ne 0 -or -not (Test-Path "$project/parity-pass.txt") -or -not (Test-Path "$project/render.png")){throw "Parity $scene failed ($($p.ExitCode)): $err Evidence: $project"}
    "PASS $scene font bounds and screenshot. Evidence: $project"
  }
} finally {foreach($name in $names){[Environment]::SetEnvironmentVariable($name,$previous[$name])}}
