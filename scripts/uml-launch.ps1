# 后台启动器：manifest 只含数据，原始参数始终按 argv 传递。
$ErrorActionPreference = 'Stop'
$manifest = $args[0]
try {
    $launch = Get-Content -LiteralPath $manifest -Raw | ConvertFrom-Json
    $owner = Get-Process -Id $PID
    $directory = Join-Path $launch.cwd '.uml-viewer'
    $null = New-Item -ItemType Directory -Force -Path $directory
    $record = Join-Path $directory 'viewer-process.json'
    # Retain the exited owner record so a subsequent restart can prove it is gone.
    # Runtime provenance records the LOVE session selected by the current wrapper.
    @{pid = $PID; started = $owner.StartTime.ToUniversalTime().Ticks.ToString(); runtime = $launch.runtime} |
        ConvertTo-Json -Compress | Set-Content -LiteralPath $record -Encoding utf8
    $log = [IO.File]::Open($launch.log, [IO.FileMode]::Append, [IO.FileAccess]::Write, [IO.FileShare]::ReadWrite)
    $writer = [IO.StreamWriter]::new($log)
    $writer.WriteLine("----- $([DateTime]::Now.ToString('yyyy-MM-dd HH:mm:ss')) starting uml-viewer")
    $writer.Flush()
    if ($launch.powershell) {
        # Bootstrap contains only a data-file path; executable/argv are read as JSON.
        $pathData = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($manifest))
        $bootstrap = "`$m=Get-Content -LiteralPath ([Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('$pathData'))) -Raw|ConvertFrom-Json;"
        $bootstrap += "Remove-Item -LiteralPath ([Text.Encoding]::UTF8.GetString([Convert]::FromBase64String('$pathData')));"
        $bootstrap += '$PSNativeCommandArgumentPassing="Standard";Set-Location -LiteralPath $m.cwd;if($m.module){Import-Module $m.module};$exe=$m.executable;$argv=@($m.argv);& $exe @argv;exit $LASTEXITCODE'
        $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($bootstrap))
        $executable = (Get-Process -Id $PID).Path
        $argv = @('-NoProfile', '-EncodedCommand', $encoded)
    } else {
        $executable = $launch.executable
        $argv = @($launch.argv)
        Remove-Item -LiteralPath $manifest
    }
    $info = [Diagnostics.ProcessStartInfo]::new($executable)
    $info.UseShellExecute = $false
    $info.WorkingDirectory = $launch.cwd
    $info.Environment['UML_VIEWER_PROJECT_ROOT'] = [IO.Path]::GetFullPath($launch.cwd)
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    foreach ($arg in $argv) { $info.ArgumentList.Add([string]$arg) }
    $process = [Diagnostics.Process]::Start($info)
    $process.StandardInput.Close()
    # 两个流并行排空，写入同一日志时由同步 writer 串行化。
    Add-Type -TypeDefinition @'
using System.IO;
using System.Threading.Tasks;
public static class UmlLog {
    public static async Task Pump(StreamReader reader, TextWriter writer) {
        string line;
        while ((line = await reader.ReadLineAsync()) != null) {
            writer.WriteLine(line);
            writer.Flush();
        }
    }
}
'@
    $sync = [IO.TextWriter]::Synchronized($writer)
    $stdout = [UmlLog]::Pump($process.StandardOutput, $sync)
    $stderr = [UmlLog]::Pump($process.StandardError, $sync)
    $process.WaitForExit()
    [Threading.Tasks.Task]::WaitAll(@($stdout, $stderr))
    $writer.Dispose()
    exit $process.ExitCode
} catch {
    # The parent exits immediately and does not retain these pipes. Persist
    # launcher errors in the same project log before any optional stderr write.
    if ($launch -and $launch.log) {
        [IO.File]::AppendAllText($launch.log, "uml launcher: $_`n")
    }
    try { [Console]::Error.WriteLine("uml launcher: $_") } catch {}
    exit 1
}
