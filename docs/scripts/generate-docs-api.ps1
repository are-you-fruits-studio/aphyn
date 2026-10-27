$ErrorActionPreference = "Stop"

function Invoke-Native {
    param(
        [Parameter(Mandatory = $true)]
        [string]$FilePath,
        [string[]]$ArgumentList = @()
    )

    & $FilePath @ArgumentList
    if ($LASTEXITCODE -ne 0) {
        throw "Command failed with exit code ${LASTEXITCODE}: $FilePath $($ArgumentList -join ' ')"
    }
}

$repoRoot = Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\..")
$targetDocDir = Join-Path $repoRoot "target\doc"
$targetMainCrateIndex = Join-Path $targetDocDir "aphyn\index.html"

Set-Location $repoRoot

$skipRustdoc = $env:SKIP_RUSTDOC -eq "1"
if (-not $skipRustdoc) {
    Invoke-Native "cargo" @("+nightly", "doc", "--workspace", "--no-deps")
} elseif (-not (Test-Path -LiteralPath $targetMainCrateIndex)) {
    throw "Cannot skip rustdoc generation because target\doc\aphyn\index.html does not exist. Run .\docs\scripts\build-and-serve.ps1 without -SkipRustdoc once."
}

if (-not (Test-Path -LiteralPath $targetDocDir)) {
    throw "Rustdoc output was not found at target\doc. Run cargo doc first."
}

if (-not (Test-Path -LiteralPath $targetMainCrateIndex)) {
    throw "Main rustdoc entry was not found: $targetMainCrateIndex"
}

Write-Host "Rustdoc API reference generated at: $targetDocDir"
