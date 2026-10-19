param([switch]$RealBinding)
$ErrorActionPreference='Stop'
# Load only installer functions; fixtures never execute its repository/tool setup.
$ast=[Management.Automation.Language.Parser]::ParseFile("$PSScriptRoot/get-uml-viewer.ps1",[ref]$null,[ref]$null)
foreach($f in $ast.FindAll({param($n) $n -is [Management.Automation.Language.FunctionDefinitionAst]},$false)) { . ([scriptblock]::Create($f.Extent.Text)) }
$root=Join-Path ([IO.Path]::GetTempPath()) ('uml-installer-deps-'+[guid]::NewGuid().ToString('N'))
$null=New-Item -ItemType Directory $root
try {
  $source=Join-Path $root 'fixture'; $null=New-Item -ItemType Directory "$source/cimgui"
  [IO.File]::WriteAllText("$source/cimgui/init.lua",'return {}')
  [IO.File]::WriteAllText("$source/cimgui.dll",'fixture DLL')
  $archive=Join-Path $root 'fixture.zip'; Compress-Archive "$source/*" $archive
  $pin=@{ Url='fixture://pinned'; ArchiveSha256=(Get-FileHash $archive).Hash; DllSha256=(Get-FileHash "$source/cimgui.dll").Hash }
  $downloads=[Collections.Generic.List[string]]::new()
  $download={param($url,$destination) if($url -ne 'fixture://pinned'){throw 'wrong pin'}; $downloads.Add($url); Copy-Item $archive $destination}.GetNewClosure()
  $target=Join-Path $root 'installed'
  foreach($iteration in 1..2){
    Install-Cimgui $target -Download $download -Pin $pin
    if(!(Test-Path "$target/cimgui/init.lua") -or !(Test-Path "$target/cimgui.dll")){throw 'incomplete clean/repeat install'}
  }
  if($downloads.Count -ne 1){throw 'repeat install downloaded again'}
  'PASS clean and repeat pinned provisioning'
  # Sarasa: real 7z fixtures through the installer's own bsdtar extraction.
  $fontSource=Join-Path $root 'font-fixture'; $null=New-Item -ItemType Directory $fontSource
  [IO.File]::WriteAllText("$fontSource/SarasaMonoSC-Regular.ttf",'fixture regular font')
  [IO.File]::WriteAllText("$fontSource/SarasaMonoSC-Bold.ttf",'fixture bold font')
  $font7z=Join-Path $root 'sarasa.7z'
  Push-Location $fontSource; try { & (Get-SystemTar) --format 7zip -cf $font7z 'SarasaMonoSC-Regular.ttf' 'SarasaMonoSC-Bold.ttf'; if($LASTEXITCODE -ne 0){throw 'could not build 7z fixture'} } finally { Pop-Location }
  $fontPin=@{ Url='fixture://sarasa'; ArchiveSha256=(Get-FileHash $font7z).Hash; FontSha256=(Get-FileHash "$fontSource/SarasaMonoSC-Regular.ttf").Hash }
  $fontDownloads=[Collections.Generic.List[string]]::new()
  $fontDownload={param($url,$destination) $fontDownloads.Add($url); Copy-Item $font7z $destination}.GetNewClosure()
  $fonts=Join-Path $root 'fonts'
  foreach($iteration in 1..2){ Install-Sarasa $fonts -Download $fontDownload -Pin $fontPin }
  if((Get-FileHash "$fonts/SarasaMonoSC-Regular.ttf").Hash -ne $fontPin.FontSha256){throw 'Sarasa Regular not installed'}
  if(Test-Path "$fonts/SarasaMonoSC-Bold.ttf"){throw 'installer extracted more than the Regular face'}
  if($fontDownloads.Count -ne 1){throw 'repeat Sarasa install downloaded again'}
  'PASS Sarasa clean and repeat provisioning from a 7z archive'
  function Assert-SarasaFails([string]$Case,[string]$Expect,[scriptblock]$Download,$Pin){
    $target=Join-Path $root ("fonts-$Case"); $failed=$null
    try { Install-Sarasa $target -Download $Download -Pin $Pin } catch { $failed=$_.Exception.Message }
    if(-not $failed){throw "Sarasa $Case did not fail"}
    if(-not $failed.Contains($Expect)){throw "Sarasa $Case failed for the wrong reason: $failed"}
    if(Test-Path "$target/SarasaMonoSC-Regular.ttf"){throw "Sarasa $Case left a font installed"}
    "PASS Sarasa $Case fails: $failed"
  }
  $seen=[Collections.Generic.List[string]]::new()
  Assert-SarasaFails 'pinned-release' 'archive SHA256 mismatch' {param($url,$destination) $seen.Add($url); [IO.File]::WriteAllText($destination,'tampered')}.GetNewClosure() $null
  if($seen[0] -ne 'https://github.com/be5invis/Sarasa-Gothic/releases/download/v1.0.42/SarasaMonoSC-TTF-1.0.42.7z'){throw "default Sarasa pin is not the fixed release: $($seen[0])"}
  'PASS default Sarasa pin is release v1.0.42'
  Assert-SarasaFails 'archive-checksum' 'archive SHA256 mismatch' $fontDownload (@{Url='fixture://sarasa'; ArchiveSha256=('0'*64); FontSha256=$fontPin.FontSha256})
  Assert-SarasaFails 'font-checksum' 'SarasaMonoSC-Regular.ttf SHA256 mismatch' $fontDownload (@{Url='fixture://sarasa'; ArchiveSha256=$fontPin.ArchiveSha256; FontSha256=('0'*64)})
  Assert-SarasaFails 'download' 'offline fixture' {param($url,$destination) throw 'offline fixture'} $fontPin
  $broken=Join-Path $root 'broken.7z'; [IO.File]::WriteAllText($broken,'not a 7z archive')
  Assert-SarasaFails 'extraction' 'extraction failed' {param($url,$destination) Copy-Item $broken $destination}.GetNewClosure() (@{Url='fixture://sarasa'; ArchiveSha256=(Get-FileHash $broken).Hash; FontSha256=$fontPin.FontSha256})
  if($RealBinding){
    # Network archive and extraction use Install-Cimgui's fresh empty directories.
    $installed=Join-Path $root '.uml-viewer/cimgui-love-1.90-1'
    Install-Cimgui $installed
    Install-Cimgui $installed
    . "$PSScriptRoot/uml-love.ps1"
    $env:UML_CIMGUI_DIRECTORY=$null
    # Exercise the public runtime entry's project-local resolution, without a
    # viewer/companion launch: an isolated LOVE fixture loads the binding.
    $env:UML_LOVE_EXECUTABLE='C:/Program Files/LOVE/lovec.exe'
    $probe=Join-Path $root 'probe'; $null=New-Item -ItemType Directory $probe
    [IO.File]::WriteAllText("$probe/main.lua",@'
function love.load()
  local d=assert(os.getenv('UML_CIMGUI_DIRECTORY')):gsub('\\','/')
  package.path=d..'/?.lua;'..d..'/?/init.lua;'..package.path
  package.cpath=d..'/?.dll;'..package.cpath
  assert(require('cimgui').C)
  love.event.quit(0)
end
'@)
    # The ir forwarding branch launches only this probe; no Herdr preflight.
    [IO.File]::WriteAllText("$root/policy.edn",'{}')
    $tool=Join-Path $root 'tool'; $null=New-Item -ItemType Directory $tool
    Copy-Item $probe "$tool/love" -Recurse
    Push-Location $root
    try { Invoke-UmlLove $root $tool @('ir'); if($LASTEXITCODE -ne 0){throw 'real cimgui module load failed'} }
    finally {Pop-Location}
    if($env:UML_CIMGUI_DIRECTORY -ne $installed){throw 'public entry did not select installed binding'}
    'PASS real pinned binding load through public runtime dependency resolution'
    # Render one frame with the actual viewer and normally exit its isolated
    # acceptance process. No companion mode or project ownership is involved.
    $env:UML_VIEWER_EVIDENCE=Join-Path $root 'render'
    $null=New-Item -ItemType Directory $env:UML_VIEWER_EVIDENCE
    $env:UML_VIEWER_TEST_SECONDS='2'
    & 'C:/Program Files/LOVE/lovec.exe' (Join-Path (Split-Path $PSScriptRoot -Parent) 'love') viewer (Join-Path (Split-Path $PSScriptRoot -Parent) 'examples/uml-viewer.edn')
    if($LASTEXITCODE -ne 0 -or !(Test-Path "$env:UML_VIEWER_EVIDENCE/render.png")){throw 'actual viewer failed with automatically provisioned binding'}
    'PASS actual viewer rendered with automatically provisioned binding'
  }
  $old=$env:PATH
  $a=Join-Path $root 'old'; $b=Join-Path $root 'compatible'; $c=Join-Path $root 'new-incompatible'
  foreach($d in @($a,$b,$c)){ $null=New-Item -ItemType Directory $d }
  [IO.File]::WriteAllText("$a/herdr.ps1",'if($args[0] -eq "--version"){"herdr 0.9.2";exit 0};throw "unexpected command"')
  [IO.File]::WriteAllText("$b/herdr.ps1",'if($args[0] -eq "--version"){"herdr 0.9.4";exit 0};if(($args -join " ") -eq "api schema --json"){''{"$schema":"fixture","protocol":22}'';exit 0};throw "unexpected command"')
  [IO.File]::WriteAllText("$c/herdr.ps1",'if($args[0] -eq "--version"){"herdr 0.9.9";exit 0};''{"$schema":"fixture","protocol":23}'';exit 0')
  try {
    $env:PATH="$a;$b"; $selected=Assert-Herdr
    if($selected.Path -ne [IO.Path]::GetFullPath("$b/herdr.ps1")){throw 'old first candidate rejected compatible installation'}
    # Equal compatible releases retain PATH order.
    Copy-Item "$b/herdr.ps1" "$a/herdr.ps1" -Force
    $env:PATH="$a;$b"; $selected=Assert-Herdr
    if($selected.Path -ne [IO.Path]::GetFullPath("$a/herdr.ps1")){throw 'equal-version PATH order changed'}
    [IO.File]::WriteAllText("$a/herdr.ps1",'if($args[0] -eq "--version"){"herdr 0.9.2";exit 0};throw "unexpected command"')
    $env:PATH="$c;$b;$a"; $selected=Assert-Herdr
    if($selected.Path -ne [IO.Path]::GetFullPath("$b/herdr.ps1")){throw 'did not fall back from incompatible highest version'}
    $env:PATH="$a;$c"; $failed=$false
    try { Assert-Herdr } catch { $failed=$_.Exception.Message.Contains('Candidates on PATH:') -and $_.Exception.Message.Contains('protocol 22') }
    if(!$failed){throw 'missing per-candidate incompatibility guidance'}
    'PASS Herdr PATH selection, schema fallback, rejection'
  } finally {$env:PATH=$old}
} finally {Remove-Item -Recurse -Force $root}
