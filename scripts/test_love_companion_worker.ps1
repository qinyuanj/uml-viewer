param([string]$Love='C:/Program Files/LOVE/lovec.exe')
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$project=Join-Path ([IO.Path]::GetTempPath()) ('uml-worker-'+[guid]::NewGuid().ToString('N'))
$null=New-Item -ItemType Directory -Path (Join-Path $project '.uml-viewer')
$names=@('UML_CIMGUI_DIRECTORY','UML_VIEWER_PROJECT_ROOT','UML_VIEWER_TEST_SECONDS','UML_VIEWER_TEST_DRIVER','CLAUDE_BIN','UML_COMPANION_NO_ATTACH'); $previous=@{}
foreach($name in $names){$previous[$name]=[Environment]::GetEnvironmentVariable($name)}
try {
  # Native harmless child accepts unknown Claude flags then exits. Recovery
  # observes its real normal/unknown exit; no user Claude session is involved.
  $fake=Join-Path $project 'claude.exe'; Copy-Item "$env:SystemRoot/System32/ping.exe" $fake
  $env:CLAUDE_BIN=$fake
  $env:UML_CIMGUI_DIRECTORY=Join-Path $repo '.uml-viewer/cimgui-love-1.90-1'
  $env:UML_VIEWER_PROJECT_ROOT=$project
  $env:UML_COMPANION_NO_ATTACH='1'
  $env:UML_VIEWER_TEST_SECONDS='90'
  $env:UML_VIEWER_TEST_DRIVER=Join-Path $repo 'scripts/test_love_companion_worker.lua'
  $info=[Diagnostics.ProcessStartInfo]::new($Love); $info.UseShellExecute=$false
  $info.WorkingDirectory=$repo; $info.RedirectStandardOutput=$true; $info.RedirectStandardError=$true
  foreach($a in @((Join-Path $repo 'love'),'viewer','--backend','herdr')){$info.ArgumentList.Add($a)}
  $p=[Diagnostics.Process]::Start($info); $out=$p.StandardOutput.ReadToEndAsync(); $err=$p.StandardError.ReadToEndAsync()
  if(-not $p.WaitForExit(100000)){throw "Viewer timeout; retained fixture $project pid $($p.Id)"}
  [IO.File]::WriteAllText("$project/stdout.txt",$out.GetAwaiter().GetResult()); [IO.File]::WriteAllText("$project/stderr.txt",$err.GetAwaiter().GetResult())
  if($p.ExitCode -ne 0){throw "Viewer exit $($p.ExitCode); fixture $project"}
  if(-not (Test-Path "$project/worker-ui-pass.txt")){throw "No ready/interaction evidence; fixture $project"}
  $deadline=[DateTime]::UtcNow.AddSeconds(35)
  do {
    $record=if(Test-Path "$project/.uml-viewer/companion.edn"){[IO.File]::ReadAllText("$project/.uml-viewer/companion.edn")}
    if($record.Trim() -eq '{}'){break}
    Start-Sleep -Milliseconds 100
  } while([DateTime]::UtcNow -lt $deadline)
  if($record.Trim() -ne '{}'){throw "Cleanup not confirmed; retained fixture $project"}
  "PASS native asynchronous Herdr startup, UI frames/context and verified shutdown. Evidence: $project"
} finally {foreach($name in $names){[Environment]::SetEnvironmentVariable($name,$previous[$name])}}
