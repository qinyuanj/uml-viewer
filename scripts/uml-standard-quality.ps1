# Scope adapter for standard tools; tool commands remain the installed defaults.
function Get-UmlBusinessRoots([string]$Root, $Standard) {
    foreach ($entry in $Standard.exclude) {
        $normalized = $entry.Replace('\', '/')
        if ([IO.Path]::IsPathRooted($entry) -or '..' -in $normalized.Split('/')) {
            throw "Invalid standard exclusion: $entry (must be project-relative without ..)."
        }
    }
    $roots = @($Standard.sourceRoots)
    if ($roots.Count -eq 0) { $roots = @('.') }
    foreach ($entry in $roots) {
        $path = [IO.Path]::GetFullPath($entry, $Root)
        $relative = [IO.Path]::GetRelativePath($Root, $path).Replace('\', '/')
        if (-not (Test-Path -LiteralPath $path -PathType Container) -or
            $relative -eq '..' -or $relative.StartsWith('../')) {
            throw "Invalid standard source root: $entry (must be an existing directory inside the project)."
        }
        $path
    }
}
function Test-UmlBusinessFile([string]$Root, [string]$Path, [array]$Roots, [array]$Exclusions) {
    $relative = [IO.Path]::GetRelativePath($Root, $Path).Replace('\', '/')
    if ($relative -eq '..' -or $relative.StartsWith('../')) { return $false }
    $included = $false
    foreach ($sourceRoot in $Roots) {
        $local = [IO.Path]::GetRelativePath($sourceRoot, $Path).Replace('\', '/')
        if ($local -ne '..' -and -not $local.StartsWith('../')) { $included = $true }
    }
    if (-not $included) { return $false }
    foreach ($entry in $Exclusions) {
        $pattern = $entry.Replace('\', '/').TrimEnd('/')
        if ($pattern.StartsWith('./')) { $pattern = $pattern.Substring(2) }
        if ([IO.Path]::IsPathRooted($entry) -or '..' -in $pattern.Split('/')) {
            throw "Invalid standard exclusion: $entry (must be project-relative without ..)."
        }
        # Whole-segment ** matches zero or more directories; a matched
        # directory also excludes all descendants.
        $segments = $pattern.Split('/')
        $regex = ''
        for ($index = 0; $index -lt $segments.Count; $index++) {
            $segment = $segments[$index]
            if ($segment -eq '**') {
                $regex += $(if ($index -eq $segments.Count - 1) { '.*' } else { '(?:[^/]+/)*' })
            } else {
                $regex += [regex]::Escape($segment).Replace('\*', '[^/]*').Replace('\?', '[^/]')
                if ($index -lt $segments.Count - 1) { $regex += '/' }
            }
        }
        if ([regex]::IsMatch($relative, '^' + $regex + '(?:/.*)?$', [Text.RegularExpressions.RegexOptions]::CultureInvariant)) { return $false }
    }
    return $true
}
function Select-UmlDryScope([string]$Root, $Standard) {
    # dryer walks source trees and skips test/spec/vendor itself, but it has no
    # exclusion flag. Without exclusions its own walker is the better selector;
    # with exclusions the business files are enumerated and named explicitly so
    # the same glob semantics as CRAP and mutation apply.
    if (@($Standard.exclude).Count -eq 0) {
        $scopeArgs = @()
        foreach ($sourceRoot in @($Standard.sourceRoots)) { if ($sourceRoot) { $scopeArgs += @('-s', $sourceRoot) } }
        return $scopeArgs
    }
    $roots = @(Get-UmlBusinessRoots $Root $Standard)
    $files = @()
    foreach ($sourceRoot in $roots) {
        foreach ($file in @(Get-ChildItem -LiteralPath $sourceRoot -File -Recurse)) {
            if ($file.Extension -cnotin @('.clj', '.cljc', '.cljs', '.cljd', '.bb', '.java', '.go', '.ts', '.tsx', '.mts', '.cts', '.rs', '.py', '.lua')) { continue }
            if (Test-UmlBusinessFile $Root $file.FullName $roots @($Standard.exclude)) {
                $files += [IO.Path]::GetRelativePath($Root, $file.FullName).Replace('\', '/')
            }
        }
    }
    if ($files.Count -eq 0) { throw 'No duplicate-analysis targets remain in the configured business scope; dryer was not started.' }
    return @($files | Sort-Object -Unique)
}
function Select-UmlMutationTargets([string]$Root, $Standard, [array]$Forward) {
    $roots = @(Get-UmlBusinessRoots $Root $Standard)
    $options = @(); $targets = @(); $changed = $false
    $takesValue = @('--root', '-s', '--source-root', '--coverage-command', '--test-command', '--busted-task', '--timeout-factor', '--mutation-warning', '--max-workers', '--lines')
    for ($index = 0; $index -lt $Forward.Count; $index++) {
        $arg = $Forward[$index]
        if ($arg -eq '--changed') { $changed = $true; continue }
        if ($arg.StartsWith('-')) {
            if ($arg -in @('--root', '-s', '--source-root')) { throw 'Standard mutation scope is configured in standardTools; do not override it in refresh arguments.' }
            $options += $arg
            if ($arg -in $takesValue) {
                $index++
                if ($index -ge $Forward.Count) { throw "$arg requires a value." }
                $options += $Forward[$index]
            }
        } else { $targets += $arg }
    }
    if ($changed) {
        $paths = @(& git -C $Root -c core.quotepath=false diff --name-only --diff-filter=AM HEAD)
        if ($LASTEXITCODE -ne 0) { throw 'Cannot select changed mutation files.' }
        $targets += $paths
        $untracked = @(& git -C $Root -c core.quotepath=false ls-files --others --exclude-standard)
        if ($LASTEXITCODE -ne 0) { throw 'Cannot select untracked mutation files.' }
        $targets += $untracked
    }
    if ($targets.Count -eq 0 -and -not $changed) { $targets = $roots }
    $files = @()
    foreach ($target in $targets) {
        $path = [IO.Path]::GetFullPath($target, $Root)
        if (-not (Test-Path -LiteralPath $path)) { throw "Mutation target does not exist: $target. Scoped refresh requires files or directories, not text filters." }
        $candidates = if (Test-Path -LiteralPath $path -PathType Container) {
            @(Get-ChildItem -LiteralPath $path -File -Recurse | ForEach-Object { $_.FullName })
        } else { @($path) }
        foreach ($file in $candidates) {
            if ([IO.File]::GetAttributes($file) -band [IO.FileAttributes]::ReparsePoint) { throw "Mutation target cannot be a linked file: $file" }
            if ([IO.Path]::GetExtension($file) -cnotin @('.clj', '.cljc', '.cljs', '.bb', '.java', '.go', '.ts', '.tsx', '.mts', '.cts', '.rs', '.py', '.lua')) { continue }
            $relativeFile = [IO.Path]::GetRelativePath($Root, $file).Replace('\', '/')
            if (@($relativeFile.Split('/') | Where-Object { $_ -in @('.git', '.uml-viewer', '.venv', 'test', 'tests', 'spec', 'specs', 'vendor', 'node_modules', 'target', '__pycache__') }).Count -gt 0) { continue }
            if (Test-UmlBusinessFile $Root $file $roots @($Standard.exclude)) {
                $files += [IO.Path]::GetRelativePath($Root, $file).Replace('\', '/')
            } else { [Console]::Error.WriteLine("uml: omitted mutation target outside business scope: $file") }
        }
    }
    $files = @($files | Select-Object -Unique)
    if ($files.Count -eq 0) { throw 'No mutation targets remain in the configured business scope; tool was not started.' }
    return $options + $files
}
