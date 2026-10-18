param(
    [string]$Love = 'C:/Program Files/LOVE/lovec.exe',
    [Parameter(Mandatory)][string]$CimguiDirectory
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path $PSScriptRoot -Parent
$evidence = Join-Path ([IO.Path]::GetTempPath()) ('uml-67-evidence-' + [guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory $evidence
# The viewer's project root is the loaded diagram's selected policy output root,
# so the isolated project root itself must hold both the diagram and mailbox.
$project = Join-Path ([IO.Path]::GetTempPath()) ('uml-67-project-' + [guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory "$project/.uml-viewer"
# Metrics root selection climbs to the nearest .metrics directory; absence falls
# back to the repository cwd. Give this isolated project its own metrics root so
# the companion mailbox resolves inside the temporary project, never the repo.
$null = New-Item -ItemType Directory "$project/.metrics"
[IO.File]::WriteAllText("$project/first.edn", '{:title "First diagram" :packages [{:label "P" :classes [{:id :target :ns "app.target"}]}]}')
[IO.File]::WriteAllText("$project/second.edn", '{:title "Second diagram" :packages [{:label "P" :classes [{:id :target :ns "app.target"}]}]}')
[IO.File]::WriteAllText("$project/.uml-viewer/to-viewer.edn", '{:next-id 1 :queue []}')
$names = @('UML_CIMGUI_DIRECTORY','UML_VIEWER_EVIDENCE','UML_VIEWER_TEST_SECONDS','UML_VIEWER_TEST_DRIVER','UML_MAIL_TEST_ROOT','UML_MAIL_TEST_TRACE')
$previous = @{}
foreach ($name in $names) { $previous[$name] = [Environment]::GetEnvironmentVariable($name) }
try {
    $env:UML_CIMGUI_DIRECTORY = (Resolve-Path $CimguiDirectory).Path
    $env:UML_VIEWER_EVIDENCE = $evidence
    $env:UML_VIEWER_TEST_SECONDS = '10'
    $env:UML_VIEWER_TEST_DRIVER = Join-Path $repo 'scripts/test_love_mail.lua'
    $env:UML_MAIL_TEST_ROOT = $project.Replace('\','/')
    $info = [Diagnostics.ProcessStartInfo]::new($Love)
    $info.UseShellExecute = $false
    $info.WorkingDirectory = $repo
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    foreach ($a in @((Join-Path $repo 'love'), 'viewer', "$project/first.edn")) { $info.ArgumentList.Add($a) }
    $p = [Diagnostics.Process]::Start($info)
    $stdout = $p.StandardOutput.ReadToEndAsync()
    $stderr = $p.StandardError.ReadToEndAsync()
    if (-not $p.WaitForExit(20000)) { throw "Owned mail process did not exit: pid $($p.Id), evidence $evidence" }
    $out = $stdout.GetAwaiter().GetResult()
    $err = $stderr.GetAwaiter().GetResult()
    [IO.File]::WriteAllText("$evidence/stdout.txt", $out)
    [IO.File]::WriteAllText("$evidence/stderr.txt", $err)
    if ($p.ExitCode -ne 0) { throw "Mail failed ($($p.ExitCode)): $err $out Evidence: $evidence" }
    if (-not (Test-Path "$evidence/native-displayed.edn")) { throw "No actual display/context evidence: $err $out $evidence" }
    $inbound = [IO.File]::ReadAllText("$project/.uml-viewer/to-viewer.edn")
    if ($inbound -notmatch ':next-id 5' -or $inbound -notmatch ':queue \[\]') { throw "Quit/context mail was not drained: $inbound" }
    $outbound = [IO.File]::ReadAllText("$project/.uml-viewer/to-agent.edn")
    if (-not (Test-Path "$evidence/native-menu-contract.edn")) { throw "No exact native menu contract evidence: $evidence" }
    # EDN semantic equality is asserted inside the actual LOVE probe.
    foreach ($op in @(':refresh-crap', ':refresh-mutate ', ':refresh-mutate-all', ':omit')) {
        if (-not $outbound.Contains($op)) { throw "Missing $op outbound mail: $outbound" }
    }
    if ($err -match 'acceptance failed|reload rejected|viewer failed|generation failed') { throw "Unexpected viewer error: $err Evidence: $evidence" }
    "PASS actual native queued display/context/normal quit and four outbound menu commands. Evidence: $evidence"
} finally {
    foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name, $previous[$name]) }
}
