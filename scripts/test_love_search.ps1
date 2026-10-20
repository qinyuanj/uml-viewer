param([string]$Love='C:/Program Files/LOVE/lovec.exe',[Parameter(Mandatory)][string]$CimguiDirectory,[Parameter(Mandatory)][string]$SarasaFont)
$ErrorActionPreference='Stop'
$repo=Split-Path $PSScriptRoot -Parent
$evidence=Join-Path ([IO.Path]::GetTempPath()) ('uml-search-'+[guid]::NewGuid().ToString('N'))
$null=New-Item -ItemType Directory $evidence
$userIni=Join-Path $env:APPDATA 'LOVE/uml-viewer/imgui.ini'
$iniBefore=if(Test-Path $userIni){@((Get-FileHash $userIni).Hash,(Get-Item $userIni).LastWriteTimeUtc.Ticks)}else{@('absent',0)}
$path=Join-Path $evidence 'diagram.edn'
[IO.File]::WriteAllText($path,'{:hierarchical true :classes [{:id :alpha.deep.parser :name "解析器" :ns "love.alpha.deep.parser"} {:id :beta.parser :name "解析器" :ns "love.beta.parser"}] :edges [{:from :alpha.deep.parser :to :beta.parser :kind :dependency}]}')
$info=[Diagnostics.ProcessStartInfo]::new($Love)
$info.UseShellExecute=$false; $info.WorkingDirectory=$repo; $info.RedirectStandardOutput=$true; $info.RedirectStandardError=$true
$info.Environment['UML_CIMGUI_DIRECTORY']=(Resolve-Path $CimguiDirectory).Path
$info.Environment['UML_CJK_FONT']=(Resolve-Path $SarasaFont).Path
$info.Environment['UML_VIEWER_PROJECT_ROOT']=$evidence
$info.Environment['UML_VIEWER_EVIDENCE']=$evidence
$info.Environment['UML_VIEWER_TEST_SECONDS']='15'
$info.Environment['UML_VIEWER_TEST_DRIVER']=Join-Path $repo 'scripts/test_love_search.lua'
foreach($a in @((Join-Path $repo 'love'),'viewer',$path)){$info.ArgumentList.Add($a)}
$p=[Diagnostics.Process]::Start($info); $stdout=$p.StandardOutput.ReadToEndAsync(); $stderr=$p.StandardError.ReadToEndAsync()
if(-not $p.WaitForExit(25000)){throw "Owned search test did not exit: pid $($p.Id), evidence $evidence"}
$out=$stdout.GetAwaiter().GetResult(); $err=$stderr.GetAwaiter().GetResult()
[IO.File]::WriteAllText("$evidence/stdout.txt",$out); [IO.File]::WriteAllText("$evidence/stderr.txt",$err)
$iniAfter=if(Test-Path $userIni){@((Get-FileHash $userIni).Hash,(Get-Item $userIni).LastWriteTimeUtc.Ticks)}else{@('absent',0)}
if(($iniBefore -join ':') -ne ($iniAfter -join ':')){throw "Native test altered user imgui.ini. Evidence: $evidence"}
if($p.ExitCode -ne 0){throw "LOVE failed ($($p.ExitCode)): $err $out Evidence: $evidence"}
foreach($file in @('search-pass.txt','search.png')){if(-not(Test-Path "$evidence/$file")){throw "Missing $file. Evidence: $evidence"}}
"PASS native cross-layer search and independent sources. Evidence: $evidence"
