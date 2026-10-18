param([switch]$Check)
# JVM-only oracle capture. Run from the repository root; never starts the viewer.
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$sourcePath = (Join-Path $repo 'src').Replace('\', '/')
$extra = '{:paths ["' + $sourcePath + '"] :deps {org.clojure/clojure {:mvn/version "1.12.0"} org.clojure/data.json {:mvn/version "2.5.1"}} :aliases {:ir {:main-opts ["-m" "uml-viewer.main.ir-generator"]}}}'
$baseline = Join-Path $repo 'fixtures/jvm-goldens'
$destination = if ($Check) { Join-Path ([System.IO.Path]::GetTempPath()) ("uml-jvm-goldens-" + [guid]::NewGuid()) } else { $baseline }
New-Item -ItemType Directory -Force $destination | Out-Null
foreach ($language in @('typescript', 'rust', 'python', 'lua')) {
    Push-Location (Join-Path $repo "$language-fixture")
    try {
        & clj -Sdeps $extra -M:ir policy.edn (Join-Path $destination "$language.ir.edn")
        if ($LASTEXITCODE -ne 0) { throw "IR generation failed: $language ($LASTEXITCODE)" }
    } finally { Pop-Location }
}
Push-Location $repo
try {
    & clj -M scripts/capture-jvm-geometry.clj $destination
    if ($LASTEXITCODE -ne 0) { throw "Geometry capture failed ($LASTEXITCODE)" }
    if ($Check) {
        foreach ($language in @('typescript', 'rust', 'python', 'lua')) {
            foreach ($kind in @('ir', 'geometry')) {
                $file = "$language.$kind.edn"
                if ((Get-FileHash (Join-Path $destination $file)).Hash -ne (Get-FileHash (Join-Path $baseline $file)).Hash) {
                    throw "Golden differs: $file (captured files retained at $destination)"
                }
            }
        }
        Remove-Item -Recurse -Force $destination -Confirm:$false
        'All eight regenerated files match byte for byte.'
    }
} finally { Pop-Location }
