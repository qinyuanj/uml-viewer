$ErrorActionPreference='Stop'
$source=[IO.File]::ReadAllText("$PSScriptRoot/test_love_viewer.ps1")
# The previous flow skips its only edit if scene-1 is absent at the deadline.
$edited=$false; $sceneReady=$false
if($sceneReady){$edited=$true}
$sceneReady=$true
if($edited){throw 'old skip fixture unexpectedly edited'}
$ast=[Management.Automation.Language.Parser]::ParseInput($source,[ref]$null,[ref]$null)
foreach($f in $ast.FindAll({param($n) $n -is [Management.Automation.Language.FunctionDefinitionAst]},$false)){ . ([scriptblock]::Create($f.Extent.Text)) }
$root=Join-Path ([IO.Path]::GetTempPath()) ('uml-reload-fixture-'+[guid]::NewGuid().ToString('N'))
$null=New-Item -ItemType Directory $root
try{
  $path=Join-Path $root 'diagram.edn'; [IO.File]::WriteAllText($path,'{:title "UML viewer"}')
  $calls=[Collections.Generic.List[string]]::new()
  $wait={param($scene,$process,$timeout)
    $calls.Add([IO.Path]::GetFileName($scene))
    if($scene.EndsWith('scene-1.edn')){
      # Scene becomes available only inside the waiter, after initial absence.
      if(Test-Path $scene){throw 'fixture must start without scene-1'}
      [IO.File]::WriteAllText($scene,'initial')
    }else{
      if(![IO.File]::ReadAllText($path).Contains('Reloaded UML viewer')){throw 'late first scene did not cause edit'}
      [IO.File]::WriteAllText($scene,'reloaded')
    }
  }.GetNewClosure()
  Invoke-UmlViewerReload $path $root ([pscustomobject]@{HasExited=$false}) $wait
  if(($calls -join ',') -ne 'scene-1.edn,scene-2.edn'){throw 'must wait for both scene publications'}
  'PASS delayed first scene always triggers exactly one edit and second wait'
}finally{Remove-Item $root -Recurse -Force}
