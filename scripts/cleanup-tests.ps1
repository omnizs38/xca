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
    "benchmark-corpus.tar", "bench-*.xca", "bench-*.zst", "bench-*.xz",
    "bench-*.lz4", "bench-*.7z", "test-zstd.zst", "test-lz4.7z"
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
