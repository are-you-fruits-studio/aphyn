$ErrorActionPreference = "Stop"

function Copy-RustdocTree {
    param(
        [Parameter(Mandatory = $true)]
        [string]$SourcePath,
        [Parameter(Mandatory = $true)]
        [string]$DestinationPath
    )

    if (Test-Path -LiteralPath $DestinationPath) {
        Remove-Item -LiteralPath $DestinationPath -Recurse -Force
    }

    New-Item -ItemType Directory -Force -Path $DestinationPath | Out-Null

    Get-ChildItem -LiteralPath $SourcePath -Force |
        Copy-Item -Destination $DestinationPath -Recurse -Force
}

$repoRoot = Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\..")
$targetDocDir = Join-Path $repoRoot "target\doc"
$buildApiDir = Join-Path $repoRoot "docs\build\api-reference"
$mainCrateIndex = Join-Path $targetDocDir "aphyn\index.html"
$buildMainCrateIndex = Join-Path $buildApiDir "aphyn\index.html"

if (-not (Test-Path -LiteralPath $mainCrateIndex)) {
    throw "Main rustdoc entry was not found: $mainCrateIndex"
}

# rustdoc output is fully self-contained with relative links (../static.files/, data-root-path="../").
# Served as a static subtree under the Docusaurus baseUrl, those relative paths resolve correctly,
# so we copy the tree verbatim with no path rewriting.
Copy-RustdocTree $targetDocDir $buildApiDir

if (-not (Test-Path -LiteralPath $buildMainCrateIndex)) {
    throw "Failed to copy rustdoc into Docusaurus build: $buildMainCrateIndex"
}

Write-Host "Rustdoc API reference copied to: $buildApiDir"
