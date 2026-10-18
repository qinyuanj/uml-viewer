param(
  [string]$Love = 'C:/Program Files/LOVE/lovec.exe',
  [string]$Lua54 = "$env:LOCALAPPDATA/Programs/Lua/bin/lua.exe"
)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$testRoot=Join-Path ([IO.Path]::GetTempPath()) ('uml57-cli-'+[guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory($testRoot) | Out-Null
$app=Join-Path $repo 'love'
function Invoke-Ir([string[]]$Arguments,[int]$Expected=0) {
  $start=[Diagnostics.ProcessStartInfo]::new($Love)
  $start.WorkingDirectory=$repo
  $start.UseShellExecute=$false
  $start.RedirectStandardOutput=$true
  $start.RedirectStandardError=$true
  foreach($a in @($app,'ir')+$Arguments) { $start.ArgumentList.Add($a) }
  $process=[Diagnostics.Process]::Start($start)
  $stdout=$process.StandardOutput.ReadToEndAsync()
  $stderr=$process.StandardError.ReadToEndAsync()
  if(-not $process.WaitForExit(10000)) {
    $null=$process.CloseMainWindow()
    throw 'Headless IR exceeded 10 seconds; requested normal window close'
  }
  $out=$stdout.GetAwaiter().GetResult(); $err=$stderr.GetAwaiter().GetResult()
  if(($Expected -eq 0 -and $process.ExitCode -ne 0) -or ($Expected -ne 0 -and $process.ExitCode -eq 0)) { throw "Unexpected exit $($process.ExitCode): $out $err" }
  if($Expected -eq 0 -and $err) { throw "Unexpected stderr: $err" }
  "exit=$($process.ExitCode) $($out.Trim()) $($err.Trim())"
}
function Put([string]$Path,[string]$Text) { [IO.File]::WriteAllText((Join-Path $testRoot $Path),$Text,[Text.UTF8Encoding]::new($false)) }
$goldenOut=Join-Path $testRoot 'golden.edn'
Invoke-Ir @('--root',(Join-Path $repo 'lua-fixture'),'--policy','policy.edn','--output',$goldenOut)
& $Lua54 (Join-Path $repo 'scripts/compare_ir.lua') $goldenOut (Join-Path $repo 'fixtures/jvm-goldens/lua.ir.edn')
if($LASTEXITCODE -ne 0) { throw 'Golden semantic comparison failed' }
Put 'policy.edn' '{:lang :lua :prefix "app" :src "src" :out "from-policy.edn" :proposals [{:id :plan :name "Plan" :notice "Keep notice" :layers [{:id :one :label "One" :nses [:a]}] :omit [:gone]}]}'
[IO.Directory]::CreateDirectory((Join-Path $testRoot 'src')) | Out-Null
Put 'src/a.lua' 'function M.go() end'
Invoke-Ir @('--root',$testRoot,'policy.edn')
if(-not [IO.File]::Exists((Join-Path $testRoot 'from-policy.edn'))) { throw 'Policy :out not used' }
$override=Join-Path $testRoot 'explicit.edn'
Invoke-Ir @('--root',$testRoot,'policy.edn',$override)
Put 'expected.edn' '{:title "UML" :hierarchical true :prefix "app" :order [] :levels [[:a]] :edge-kinds {} :omit-edges [] :omit [] :classes [{:id :a :name "A" :ns "app.a" :lang :lua :file "src/a.lua" :ops [{:name "M.go" :text "M.go"}] :level 0}] :edges [] :policy-file "policy.edn" :proposals [{:id :plan :name "Plan" :notice "Keep notice" :layers [{:id :one :label "One" :nses [:a]}] :omit [:gone]}] :proposal {:notice "Keep notice" :layers [{:id :one :label "One" :nses [:a]}] :omit [:gone]}}'
& $Lua54 (Join-Path $repo 'scripts/compare_ir.lua') $override (Join-Path $testRoot 'expected.edn')
if($LASTEXITCODE -ne 0) { throw 'Proposal preservation comparison failed' }
foreach($case in @(
  @{Name='malformed';Policy='{:lang :lua';Args=@()},
  @{Name='missing-source';Policy='{:lang :lua :src "absent"}';Args=@()},
  @{Name='unknown-language';Policy='{:lang :unknown}';Args=@()},
  @{Name='wrong-policy-type';Policy='[]';Args=@()},
  @{Name='unknown-option';Policy='{:lang :lua}';Args=@('--wat')}
)) {
  Put ($case.Name+'.edn') $case.Policy
  $out=Join-Path $testRoot ($case.Name+'-output.edn')
  Invoke-Ir (@('--root',$testRoot,($case.Name+'.edn'),$out)+$case.Args) 1
  if([IO.File]::Exists($out)) { throw "Failure wrote partial output: $($case.Name)" }
}
Invoke-Ir @('--root',$testRoot,'policy.edn',(Join-Path $testRoot 'absent/out.edn')) 1
"Headless LOVE CLI checks passed; artifacts: $testRoot"
