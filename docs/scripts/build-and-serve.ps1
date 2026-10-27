param(
    [string]$HostName = "127.0.0.1",
    [int]$Port = 3000,
    [switch]$SkipNpmInstall,
    [switch]$ForceNpmInstall,
    [switch]$SkipRustdoc,
    [switch]$NoServe
)

$ErrorActionPreference = "Stop"

function Resolve-CommandPath {
    param(
        [Parameter(Mandatory = $true)]
        [string]$CommandName,
        [string[]]$FallbackPaths = @()
    )

    $command = Get-Command $CommandName -ErrorAction SilentlyContinue
    if ($command) {
        return $command.Source
    }

    foreach ($path in $FallbackPaths) {
        if (Test-Path -LiteralPath $path) {
            return $path
        }
    }

    throw "Required command '$CommandName' was not found."
}

function Invoke-Native {
    param(
        [Parameter(Mandatory = $true)]
        [string]$FilePath,
        [Parameter(ValueFromRemainingArguments = $true)]
        [string[]]$Arguments
    )

    & $FilePath @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Command failed with exit code ${LASTEXITCODE}: $FilePath $($Arguments -join ' ')"
    }
}

function Get-ListeningProcessOnPort {
    param(
        [Parameter(Mandatory = $true)]
        [int]$Port
    )

    return Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue |
        Select-Object -First 1 -ExpandProperty OwningProcess
}

$repoRoot = Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..\..")
$docsRoot = Join-Path $repoRoot "docs"
$powershell = Resolve-CommandPath "powershell" @("C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe")
$npm = Resolve-CommandPath "npm" @("C:\Program Files\nodejs\npm.cmd")
$nodeDir = Split-Path -Parent $npm
$env:PATH = "$nodeDir;$env:PATH"

Set-Location $repoRoot

Write-Host "==> Checking Rust docs tooling"
rustup toolchain install nightly
if ($LASTEXITCODE -ne 0) {
    throw "Failed to install or update Rust nightly toolchain."
}

Write-Host "==> Generating Rust API reference"
if ($SkipRustdoc) {
    $env:SKIP_RUSTDOC = "1"
} else {
    Remove-Item Env:\SKIP_RUSTDOC -ErrorAction SilentlyContinue
}

Invoke-Native $powershell "-ExecutionPolicy" "Bypass" "-File" (Join-Path $docsRoot "scripts\generate-docs-api.ps1")

Set-Location $docsRoot

$nodeModules = Join-Path $docsRoot "node_modules"
$docusaurusBin = Join-Path $nodeModules ".bin\docusaurus.cmd"

if ($SkipNpmInstall -and -not (Test-Path -LiteralPath $nodeModules)) {
    throw "docs/node_modules is missing. Run without -SkipNpmInstall first."
}

if (-not $SkipNpmInstall -and ($ForceNpmInstall -or -not (Test-Path -LiteralPath $docusaurusBin))) {
    Write-Host "==> Installing Docusaurus dependencies"
    Invoke-Native $npm "install"
} elseif (-not $SkipNpmInstall) {
    Write-Host "==> Docusaurus dependencies already installed"
}

Write-Host "==> Building Docusaurus static HTML"
Invoke-Native $npm "run" "build"

Write-Host "==> Copying rustdoc API reference into Docusaurus build"
Invoke-Native $powershell "-ExecutionPolicy" "Bypass" "-File" (Join-Path $docsRoot "scripts\copy-rustdoc-to-build.ps1")

if ($NoServe) {
    Write-Host "==> Build complete. Skipping local server because -NoServe was set."
    exit 0
}

$docsUrl = "http://$HostName`:$Port/aphyn/"

$existingProcessId = Get-ListeningProcessOnPort $Port
if ($existingProcessId) {
    $existingProcess = Get-Process -Id $existingProcessId -ErrorAction SilentlyContinue
    $processName = if ($existingProcess) { $existingProcess.ProcessName } else { "unknown" }

    Write-Host "==> Port $Port is already in use by PID $existingProcessId ($processName)."
    Write-Host "Stop it first:"
    Write-Host "    Stop-Process -Id $existingProcessId"
    Write-Host "Or run this script with another port:"
    Write-Host "    .\docs\scripts\build-and-serve.ps1 -Port 3001"
    exit 1
}

Write-Host "==> Starting local docs server in this terminal"
Write-Host "Open: $docsUrl"
Write-Host "Press Ctrl+C in this terminal to stop it."
# Use the GitHub Pages-faithful static server instead of `docusaurus serve`, which
# 301-mangles rustdoc's relative *.html navigation. See serve-build.mjs for details.
$node = Resolve-CommandPath "node" @("C:\Program Files\nodejs\node.exe")
Invoke-Native $node (Join-Path $docsRoot "scripts\serve-build.mjs") "--host" $HostName "--port" "$Port"
