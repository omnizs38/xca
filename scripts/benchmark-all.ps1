param(
    [Parameter(Mandatory = $true)]
    [string[]]$InputPath,
    [ValidateRange(3, 99)]
    [int]$Iterations = 7,
    [string]$OutputPrefix = "strict-benchmark-results",
    [bool]$RequireAll = $true,
    [switch]$KeepArtifacts
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Resolve-Executable {
    param([string]$Command, [string]$Fallback)
    $found = Get-Command $Command -ErrorAction SilentlyContinue
    if ($found) { return $found.Source }
    if ($Fallback -and (Test-Path -LiteralPath $Fallback)) {
        return (Resolve-Path -LiteralPath $Fallback).Path
    }
    return $null
}

function Invoke-Checked {
    param([scriptblock]$Command, [string]$Label)
    # Windows PowerShell turns native stderr into an ErrorRecord under Stop.
    # Codecs legitimately print progress there, so judge them by exit status
    # and restore strict PowerShell error handling immediately afterward.
    $previousPreference = $ErrorActionPreference
    $exitCode = -1
    try {
        $ErrorActionPreference = "Continue"
        & $Command
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousPreference
    }
    if ($exitCode -ne 0) {
        throw "$Label failed with exit code $exitCode"
    }
}

function Get-Median {
    param([double[]]$Values)
    $sorted = @($Values | Sort-Object)
    return $sorted[[int][Math]::Floor($sorted.Count / 2)]
}

function Measure-Codec {
    param(
        [string]$Corpus,
        [string]$Name,
        [string]$ArchivePath,
        [string]$RestoredPath,
        [scriptblock]$Compress,
        [scriptblock]$Decompress,
        [long]$SourceSize,
        [string]$SourceHash
    )

    Remove-Item -LiteralPath $ArchivePath -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $RestoredPath -Force -Recurse -ErrorAction SilentlyContinue
    Invoke-Checked $Compress "$Name compression warm-up"
    if (-not (Test-Path -LiteralPath $ArchivePath -PathType Leaf)) {
        throw "$Name did not create its archive"
    }
    Invoke-Checked $Decompress "$Name decompression warm-up"
    if (-not (Test-Path -LiteralPath $RestoredPath -PathType Leaf)) {
        throw "$Name did not create its restored file"
    }
    $warmHash = (Get-FileHash -LiteralPath $RestoredPath -Algorithm SHA256).Hash
    if ($warmHash -ne $SourceHash) {
        throw "$Name failed SHA-256 verification during warm-up"
    }

    $encodeTimes = foreach ($run in 1..$Iterations) {
        Remove-Item -LiteralPath $ArchivePath -Force -ErrorAction SilentlyContinue
        $elapsed = (Measure-Command {
            Invoke-Checked $Compress "$Name compression run $run"
        }).TotalMilliseconds
        if (-not (Test-Path -LiteralPath $ArchivePath -PathType Leaf)) {
            throw "$Name did not create an archive on run $run"
        }
        $elapsed
    }

    $archiveSize = (Get-Item -LiteralPath $ArchivePath).Length
    $decodeTimes = foreach ($run in 1..$Iterations) {
        Remove-Item -LiteralPath $RestoredPath -Force -Recurse -ErrorAction SilentlyContinue
        $elapsed = (Measure-Command {
            Invoke-Checked $Decompress "$Name decompression run $run"
        }).TotalMilliseconds
        if (-not (Test-Path -LiteralPath $RestoredPath -PathType Leaf)) {
            throw "$Name did not restore a file on run $run"
        }
        $hash = (Get-FileHash -LiteralPath $RestoredPath -Algorithm SHA256).Hash
        if ($hash -ne $SourceHash) {
            throw "$Name failed SHA-256 verification on decode run $run"
        }
        $elapsed
    }

    $encodeMedian = Get-Median $encodeTimes
    $decodeMedian = Get-Median $decodeTimes
    $sourceMiB = $SourceSize / 1MB
    [PSCustomObject]@{
        Corpus = $Corpus
        Engine = $Name
        SourceBytes = $SourceSize
        ArchiveBytes = $archiveSize
        RatioPercent = [Math]::Round($archiveSize / $SourceSize * 100, 4)
        SavedPercent = [Math]::Round((1 - $archiveSize / $SourceSize) * 100, 4)
        EncodeMedianMs = [Math]::Round($encodeMedian, 3)
        EncodeMiBs = [Math]::Round($sourceMiB / ($encodeMedian / 1000), 2)
        DecodeMedianMs = [Math]::Round($decodeMedian, 3)
        DecodeMiBs = [Math]::Round($sourceMiB / ($decodeMedian / 1000), 2)
        Iterations = $Iterations
        VerifiedSHA256 = $true
    }
}

$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$work = Join-Path $root ".xca-benchmark"
New-Item -ItemType Directory -Path $work -Force | Out-Null

$xcaExe = Resolve-Executable "xca.exe" (Join-Path $root "target\release\xca.exe")
$zstdExe = Resolve-Executable "zstd.exe" $null
$sevenZip = Resolve-Executable "7z.exe" "$env:ProgramFiles\7-Zip\7z.exe"
$sevenZipZSPath = "$env:ProgramFiles\7-Zip-Zstandard\7z.exe"
$sevenZipZS = if (Test-Path -LiteralPath $sevenZipZSPath) {
    (Resolve-Path -LiteralPath $sevenZipZSPath).Path
} else {
    $null
}

$missing = @()
if (-not $xcaExe) { $missing += "XCA" }
if (-not $zstdExe) { $missing += "Zstandard" }
if (-not $sevenZipZS) { $missing += "7-Zip ZS/LZ4" }
if (-not $sevenZip) { $missing += "7-Zip/LZMA2" }
if ($RequireAll -and $missing.Count -gt 0) {
    throw "Required engines are missing: $($missing -join ', ')"
}
if (-not $xcaExe) { throw "XCA release executable is missing; run cargo build --release first" }

$results = @()
foreach ($inputItem in $InputPath) {
    $source = (Resolve-Path -LiteralPath $inputItem).Path
    $sourceItem = Get-Item -LiteralPath $source
    if ($sourceItem.Length -eq 0) { throw "Benchmark input must not be empty: $source" }
    $sourceSize = $sourceItem.Length
    $sourceHash = (Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash
    $corpus = $sourceItem.Name
    $safe = ($corpus -replace '[^A-Za-z0-9_.-]', '_')

    foreach ($level in @(1, 5, 9)) {
        $archive = Join-Path $work "$safe.xca-l$level.xca"
        $restored = Join-Path $work "$safe.xca-l$level.restored"
        $results += Measure-Codec $corpus "XCA level $level" $archive $restored {
            & $xcaExe compress $source $archive $level 2>$null
        } {
            & $xcaExe decompress $archive $restored 2>$null
        } $sourceSize $sourceHash
    }

    if ($zstdExe) {
        foreach ($level in @(1, 3, 9)) {
            $archive = Join-Path $work "$safe.zstd-l$level.zst"
            $restored = Join-Path $work "$safe.zstd-l$level.restored"
            $results += Measure-Codec $corpus "Zstandard level $level T0" $archive $restored {
                & $zstdExe "-$level" -T0 -q -f $source -o $archive
            } {
                & $zstdExe -d -q -f $archive -o $restored
            } $sourceSize $sourceHash
        }
    }

    if ($sevenZipZS) {
        $archive = Join-Path $work "$safe.lz4.7z"
        $restoreDir = Join-Path $work "$safe.lz4-out"
        $restored = Join-Path $restoreDir $sourceItem.Name
        $results += Measure-Codec $corpus "LZ4 7-Zip ZS" $archive $restored {
            & $sevenZipZS a -t7z -m0=LZ4 -mx=1 -mmt=on -y $archive $source | Out-Null
        } {
            Remove-Item -LiteralPath $restoreDir -Force -Recurse -ErrorAction SilentlyContinue
            New-Item -ItemType Directory -Path $restoreDir -Force | Out-Null
            & $sevenZipZS e -y "-o$restoreDir" $archive | Out-Null
        } $sourceSize $sourceHash
    }

    if ($sevenZip) {
        $archive = Join-Path $work "$safe.lzma2.7z"
        $restoreDir = Join-Path $work "$safe.lzma2-out"
        $restored = Join-Path $restoreDir $sourceItem.Name
        $results += Measure-Codec $corpus "LZMA2 level 5" $archive $restored {
            & $sevenZip a -t7z -m0=LZMA2 -mx=5 -mmt=on -y $archive $source | Out-Null
        } {
            Remove-Item -LiteralPath $restoreDir -Force -Recurse -ErrorAction SilentlyContinue
            New-Item -ItemType Directory -Path $restoreDir -Force | Out-Null
            & $sevenZip e -y "-o$restoreDir" $archive | Out-Null
        } $sourceSize $sourceHash
    }
}

$csvPath = Join-Path $root "$OutputPrefix.csv"
$jsonPath = Join-Path $root "$OutputPrefix.json"
$metadataPath = Join-Path $root "$OutputPrefix.metadata.json"
$results | Export-Csv -LiteralPath $csvPath -NoTypeInformation -Encoding UTF8
$results | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $jsonPath -Encoding UTF8
[PSCustomObject]@{
    TimestampUtc = [DateTime]::UtcNow.ToString("o")
    Computer = $env:COMPUTERNAME
    LogicalProcessors = [Environment]::ProcessorCount
    OS = [Environment]::OSVersion.VersionString
    Iterations = $Iterations
    Inputs = @($InputPath)
    XCA = $xcaExe
    Zstandard = $zstdExe
    SevenZip = $sevenZip
    SevenZipZS = $sevenZipZS
    MissingEngines = $missing
} | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath $metadataPath -Encoding UTF8

$results |
    Sort-Object Corpus, RatioPercent, EncodeMiBs -Descending:$false |
    Format-Table Corpus, Engine, ArchiveBytes, RatioPercent, EncodeMiBs, DecodeMiBs, VerifiedSHA256

Write-Host "CSV: $csvPath"
Write-Host "JSON: $jsonPath"
Write-Host "Metadata: $metadataPath"

if (-not $KeepArtifacts) {
    Remove-Item -LiteralPath $work -Force -Recurse -ErrorAction SilentlyContinue
}
