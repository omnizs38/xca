param(
    [string[]]$BenchmarkInput,
    [ValidateRange(3, 99)]
    [int]$Iterations = 7,
    [switch]$SkipBenchmark
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Push-Location $root
try {
    cargo fmt --all -- --check
    if ($LASTEXITCODE -ne 0) { throw "cargo fmt failed" }

    cargo clippy --all-targets --all-features -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw "cargo clippy failed" }

    cargo test --all-targets --all-features
    if ($LASTEXITCODE -ne 0) { throw "cargo test failed" }

    & (Join-Path $PSScriptRoot "build-release.ps1") -Native
    if ($LASTEXITCODE -ne 0) { throw "release build failed" }

    if (-not $SkipBenchmark) {
        if (-not $BenchmarkInput -or $BenchmarkInput.Count -eq 0) {
            throw "Provide -BenchmarkInput or use -SkipBenchmark"
        }
        & (Join-Path $PSScriptRoot "benchmark-all.ps1") `
            -InputPath $BenchmarkInput `
            -Iterations $Iterations `
            -RequireAll $true
        if ($LASTEXITCODE -ne 0) { throw "cross-engine benchmark failed" }
    }

    Write-Host "All strict XCA checks passed."
}
finally {
    Pop-Location
}
