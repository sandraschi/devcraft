# DevCraft start: launches native desktop app (egui 0.37)
param(
    [switch]$Build = $false,
    [switch]$Smoke = $false
)
$ErrorActionPreference = "Stop"

$env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
if ($null -eq (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "ERROR: cargo not found." -ForegroundColor Red
    exit 1
}

Set-Location -LiteralPath $PSScriptRoot

if ($Smoke) {
    & cargo run -p devcraft-app -- --smoke
    exit $LASTEXITCODE
}

if ($Build) {
    & cargo build --workspace
}

& cargo run -p devcraft-app
