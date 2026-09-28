param(
    [switch]$DeleteBuild,
    [switch]$Force
)

$patterns = @(
    "input.txt", "output.xca", "restored.txt",
    "powershell-test.exe", "powershell-test.xca", "powershell-restored.exe",
    "random.bin", "random.xca", "random-restored.bin",
    "level-*.xca", "xca-corpus.tar", "corpus-level-*.xca",
    "large-corpus.tar", "large-level-*.xca", "large-restored.tar",
    "benchmark-corpus.tar", "benchmark-large.bin", "benchmark-level-*.xca", "benchmark-restored.tar", "bench-*.xca", "bench-*.zst", "bench-*.xz",
    "bench-*.lz4", "large-xca-*.xca", "large-lz4.7z", "large-zstd-*.zst", "bench-*.7z", "test-zstd.zst", "test-lz4.7z",
    "xca08-*.xca", "xca08-restored.bin", "xca7-*.xca", "xca7-*.bin", "xca7-*.out", "xca8-*.xca", "xca8-*.bin", "xca8-*.out", "strict-benchmark-results.csv", "strict-benchmark-results.json", "strict-benchmark-results.metadata.json"
)

$files = Get-ChildItem -Path $patterns -File -ErrorAction SilentlyContinue |
    Sort-Object FullName -Unique

if (-not $files) {
    Write-Host "No XCA test artifacts found."
} else {
    $files | Select-Object Name, Length | Format-Table
    if (-not $Force) {
        $answer = Read-Host "Type DELETE to remove these files"
        if ($answer -ne "DELETE") { Write-Host "Cancelled."; exit 0 }
    }
    $files | Remove-Item -Force
    Write-Host "Removed $($files.Count) test artifact(s)."
}

if ($DeleteBuild) {
    cargo clean
}

$benchmarkDirectory = Join-Path $PSScriptRoot "..\.xca-benchmark"
if (Test-Path -LiteralPath $benchmarkDirectory) {
    Remove-Item -LiteralPath $benchmarkDirectory -Force -Recurse
    Write-Host "Removed .xca-benchmark directory."
}
