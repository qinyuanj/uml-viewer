# Wrapper-level coverage for LOVE `--runtime love ir` default-policy selection.
# Each case is an isolated installed project: the real LOVE app is copied under
# .uml-viewer/uml-viewer/love so no repository or companion state is touched.
param(
  [string]$Love = 'C:/Program Files/LOVE/lovec.exe'
)
$ErrorActionPreference='Stop'
if(-not (Test-Path -LiteralPath $Love)) { throw "LOVE console executable not found: $Love" }
$repo=Split-Path $PSScriptRoot -Parent
$testRoot=Join-Path ([IO.Path]::GetTempPath()) ('uml76-ir-policy-'+[guid]::NewGuid().ToString('N'))
$tool=Join-Path $testRoot '.uml-viewer/uml-viewer'
[IO.Directory]::CreateDirectory($tool) | Out-Null
Copy-Item -LiteralPath (Join-Path $repo 'love') -Destination (Join-Path $tool 'love') -Recurse
$leaf=Split-Path $testRoot -Leaf
$env:UML_LOVE_EXECUTABLE=$Love
function Put([string]$Path,[string]$Text) {
  $full=Join-Path $testRoot $Path
  [IO.Directory]::CreateDirectory((Split-Path $full -Parent)) | Out-Null
  [IO.File]::WriteAllText($full,$Text,[Text.UTF8Encoding]::new($false))
}
function Invoke-Wrapper {
  $start=[Diagnostics.ProcessStartInfo]::new((Get-Process -Id $PID).Path)
  $start.WorkingDirectory=$testRoot
  $start.UseShellExecute=$false
  $start.RedirectStandardOutput=$true
  $start.RedirectStandardError=$true
  foreach($a in @('-NoProfile','-File',(Join-Path $repo 'scripts/uml-command.ps1'),$testRoot,'--runtime','love','ir')) { $start.ArgumentList.Add($a) }
  $process=[Diagnostics.Process]::Start($start)
  $stdout=$process.StandardOutput.ReadToEndAsync(); $stderr=$process.StandardError.ReadToEndAsync()
  if(-not $process.WaitForExit(20000)) {
    $null=$process.CloseMainWindow()
    throw 'Headless LOVE ir exceeded 20 seconds; requested normal window close'
  }
  @{ Exit=$process.ExitCode; Out=$stdout.GetAwaiter().GetResult(); Err=$stderr.GetAwaiter().GetResult() }
}
function Clear-Outputs {
  Get-ChildItem -LiteralPath $testRoot -Filter 'chosen-*.edn' -File -ErrorAction SilentlyContinue | Remove-Item -Force
  $default=Join-Path $testRoot 'examples/uml-viewer.edn'
  if(Test-Path -LiteralPath $default) { Remove-Item -LiteralPath $default -Force }
}
function Assert-Selected([string]$Case,[string]$ExpectedOut,[string]$ExpectedPolicy) {
  Clear-Outputs
  $result=Invoke-Wrapper
  if($result.Exit -ne 0) { throw "${Case}: unexpected exit $($result.Exit): $($result.Out) $($result.Err)" }
  $expected=Join-Path $testRoot $ExpectedOut
  if(-not [IO.File]::Exists($expected)) { throw "${Case}: expected $ExpectedOut; stdout=$($result.Out) stderr=$($result.Err)" }
  $written=@(Get-ChildItem -LiteralPath $testRoot -Filter 'chosen-*.edn' -File | ForEach-Object Name)
  $expectedLeaf=Split-Path $ExpectedOut -Leaf
  $selected=if($expectedLeaf -like 'chosen-*.edn') { @($expectedLeaf) } else { @() }
  if(($written -join "`n") -cne ($selected -join "`n")) {
    throw "${Case}: wrote [$($written -join ', ')], expected only [$($selected -join ', ')]"
  }
  $text=[IO.File]::ReadAllText($expected)
  $match=[regex]::Match($text,':policy-file "((?:[^"\\]|\\.)*)"')
  if(-not $match.Success) { throw "${Case}: $ExpectedOut has no :policy-file" }
  $recorded=($match.Groups[1].Value -replace '\\\\','\')
  if([IO.Path]::GetFileName($recorded) -cne $ExpectedPolicy) {
    throw "${Case}: selected policy '$recorded', expected $ExpectedPolicy"
  }
  "${Case}: $ExpectedOut <- $ExpectedPolicy"
}
function Get-ProjectFiles {
  @(Get-ChildItem -LiteralPath $testRoot -Recurse -File |
    Where-Object { $_.FullName -notmatch '[\\/]\.uml-viewer[\\/]' } |
    ForEach-Object { $_.FullName.Substring($testRoot.Length).Replace('\','/') } | Sort-Object)
}
try {
  Put 'src/a.lua' 'function M.go() end'
  Put "examples/$leaf.policy.edn" '{:lang :lua :src "src" :out "chosen-project.edn"}'
  Put 'examples/a.policy.edn' '{:lang :lua :src "src" :out "chosen-sorted.edn"}'
  Put 'examples/z.policy.edn' '{:lang :lua :src "src" :out "chosen-z.edn"}'
  Put 'policy.edn' '{:lang :lua :src "src" :out "chosen-root.edn"}'

  Assert-Selected 'project-named policy wins' 'chosen-project.edn' "$leaf.policy.edn"
  Remove-Item -LiteralPath (Join-Path $testRoot "examples/$leaf.policy.edn") -Force

  Assert-Selected 'sorted examples policy next' 'chosen-sorted.edn' 'a.policy.edn'
  Remove-Item -LiteralPath (Join-Path $testRoot 'examples/a.policy.edn') -Force
  Remove-Item -LiteralPath (Join-Path $testRoot 'examples/z.policy.edn') -Force

  Assert-Selected 'root policy next' 'chosen-root.edn' 'policy.edn'

  # An empty policy has no :out, so IR generation uses its built-in default.
  Remove-Item -LiteralPath (Join-Path $testRoot 'policy.edn') -Force
  Put 'policy.edn' '{}'
  Assert-Selected 'empty policy falls through to the IR default output' 'examples/uml-viewer.edn' 'policy.edn'

  # No candidate at any level: the wrapper must diagnose and write nothing.
  Remove-Item -LiteralPath (Join-Path $testRoot 'policy.edn') -Force
  Remove-Item -LiteralPath (Join-Path $testRoot 'examples/uml-viewer.edn') -Force
  $before=Get-ProjectFiles
  $result=Invoke-Wrapper
  if($result.Exit -eq 0) { throw "no-policy diagnostic: expected nonzero exit; stdout=$($result.Out)" }
  if($result.Err -notmatch 'no policy') { throw "no-policy diagnostic: stderr=$($result.Err)" }
  $after=Get-ProjectFiles
  if(($after -join "`n") -cne ($before -join "`n")) {
    throw "no-policy diagnostic wrote output: before=[$($before -join ', ')] after=[$($after -join ', ')]"
  }
  "no-policy diagnostic: exit=$($result.Exit), no output written"
  "LOVE ir default-policy selection checks passed; artifacts: $testRoot"
} finally {
  Remove-Item Env:UML_LOVE_EXECUTABLE -ErrorAction SilentlyContinue
}
