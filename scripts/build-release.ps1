param(
    [switch]$NoDefaultFeatures,
    [switch]$Native
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest
$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$features = if ($NoDefaultFeatures) { @("--no-default-features") } else { @("--all-features") }
$previousRustFlags = $env:RUSTFLAGS

Push-Location $root
try {
    if ($Native) {
        $env:RUSTFLAGS = "$previousRustFlags -C target-cpu=native".Trim()
    }
    # Building the library and executable as separate target selections avoids
    # Cargo's same-package xca.pdb filename collision on MSVC.
    & cargo build --release @features --lib
    if ($LASTEXITCODE -ne 0) { throw "release library build failed" }

    & cargo build --release @features --bin xca
    if ($LASTEXITCODE -ne 0) { throw "release CLI build failed" }

    Write-Host "Release library and CLI built without cross-target PDB collisions."
}
finally {
    $env:RUSTFLAGS = $previousRustFlags
    Pop-Location
}