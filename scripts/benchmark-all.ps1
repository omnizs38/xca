param(
    [Parameter(Mandatory = $true)]
    [string[]]$InputPath,
    [ValidateRange(1, 99)]
    [int]$Iterations = 7
)
$ErrorActionPreference = "Stop"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$args = @((Join-Path $root "scripts\bench.py"), "run", "--suite", "quick", "--iterations", $Iterations)
foreach ($path in $InputPath) { $args += @("--input", $path) }
& python @args
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
