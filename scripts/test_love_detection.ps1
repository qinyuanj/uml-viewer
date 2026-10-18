param([string]$Love='C:/Program Files/LOVE/lovec.exe',[switch]$AllLanguages)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$fixture=Join-Path ([IO.Path]::GetTempPath()) ('uml61-detection-'+[guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory((Join-Path $fixture 'src')) | Out-Null
function Put([string]$Path,[string]$Text) { [IO.File]::WriteAllText((Join-Path $fixture $Path),$Text,[Text.UTF8Encoding]::new($false)) }
function Run([string]$Name,[string]$Policy,[string]$ErrorText='') {
  Put 'policy.edn' $Policy
  $output=Join-Path $fixture ($Name+'.edn')
  $start=[Diagnostics.ProcessStartInfo]::new($Love)
  $start.WorkingDirectory=$repo; $start.UseShellExecute=$false
  $start.RedirectStandardOutput=$true; $start.RedirectStandardError=$true
  foreach($arg in @((Join-Path $repo 'love'),'ir','--root',$fixture,'policy.edn',$output)) { $start.ArgumentList.Add($arg) }
  $process=[Diagnostics.Process]::Start($start)
  $stdout=$process.StandardOutput.ReadToEndAsync(); $stderr=$process.StandardError.ReadToEndAsync()
  if(-not $process.WaitForExit(10000)) { $null=$process.CloseMainWindow(); throw 'Headless detection CLI timeout' }
  $text=$stdout.GetAwaiter().GetResult()+$stderr.GetAwaiter().GetResult()
  if($ErrorText) {
    if($process.ExitCode -eq 0 -or $text -notlike ('*'+$ErrorText+'*') -or [IO.File]::Exists($output)) { throw "${Name}: expected nonzero, diagnostic $ErrorText, and no output; exit=$($process.ExitCode) $text" }
  } elseif($process.ExitCode -ne 0 -or -not [IO.File]::Exists($output)) { throw "${Name}: exit=$($process.ExitCode) $text" }
  "${Name}: exit=$($process.ExitCode)"
}
Run 'no-markers' '{}' ':lang or :sources'
Put 'src/a.lua' 'function M.run() end'
Run 'lua-marker' '{:prefix "app"}'
Put 'package.json' '{}'
Run 'ambiguous' '{}' ':lang or :sources'
Run 'explicit-lang' '{:lang :lua}'
Run 'explicit-sources' '{:sources [{:lang :lua :root "src"}]}'
Run 'clojure' '{:lang :clojure}' 'Clojure is not supported'
Run 'source-clojure' '{:sources [{:lang :clojure}]}' 'Clojure is not supported'
Run 'unknown' '{:lang :brainfuck}' 'no LanguageGraph for :brainfuck'
if($AllLanguages) {
  foreach($language in @('typescript','rust','python','lua')) {
    $fixture=Join-Path ([IO.Path]::GetTempPath()) ('uml61-'+$language+'-'+[guid]::NewGuid().ToString('N'))
    [IO.Directory]::CreateDirectory($fixture) | Out-Null
    Copy-Item -Path (Join-Path $repo ($language+'-fixture/*')) -Destination $fixture -Recurse
    Run ($language+'-detected') '{:src "src" :prefix "app"}'
    $document=[IO.File]::ReadAllText((Join-Path $fixture ($language+'-detected.edn')))
    if($document -notmatch (':lang\s+:'+ $language +'\b') -or $document -notmatch ':classes\s+\[\s*\{') { throw "$language detection did not produce scanned classes" }
  }
  'All four marker fixtures passed actual headless LOVE CLI'
}
"Detection LOVE CLI checks passed; artifacts: $fixture"
