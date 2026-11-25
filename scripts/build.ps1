# Builds the portable release exe and copies it to a distribution folder.
#
# Usage:
#   powershell -ExecutionPolicy Bypass -File scripts\build.ps1
#   powershell -ExecutionPolicy Bypass -File scripts\build.ps1 -Destination E:\
#
# The output is a single exe with no installer, no DLLs and no config files,
# ready to be dropped onto a memory card.

param(
    [string]$Destination = ""
)

$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

Write-Host "Building release..." -ForegroundColor Cyan
cargo build --release
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

$exe = Join-Path (Get-Location) "target\release\Unprompted.exe"
$sizeMb = [math]::Round((Get-Item $exe).Length / 1MB, 2)
Write-Host "Built $exe ($sizeMb MB)" -ForegroundColor Green

if ($Destination -ne "") {
    if (-not (Test-Path $Destination)) { throw "destination not found: $Destination" }
    Copy-Item $exe -Destination $Destination -Force
    Write-Host "Copied to $Destination" -ForegroundColor Green
}
