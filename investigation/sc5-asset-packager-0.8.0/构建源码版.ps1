param([string]$Python = "python")
$ErrorActionPreference = "Stop"
Push-Location $PSScriptRoot
try {
    & $Python "build_source.py"
    if ($LASTEXITCODE -ne 0) { throw "Build failed: $LASTEXITCODE" }
    Write-Host "EXE: $PSScriptRoot\release\0.8.0\SC5 Asset Packager.exe"
} finally { Pop-Location }
