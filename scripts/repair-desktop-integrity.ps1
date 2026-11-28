# Repair only disposable build artifacts; never lower database integrity.
$ErrorActionPreference = 'Stop'
$projectRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$buildRoot = [IO.Path]::GetFullPath((Join-Path $projectRoot 'target'))
if ($buildRoot -ne (Join-Path $projectRoot 'target')) { throw 'Unexpected build path' }
if (-not (Test-Path -LiteralPath $buildRoot -PathType Container)) { throw 'No target directory' }
# Refuse redirected output trees before changing security labels recursively.
$redirected = @(Get-Item -LiteralPath $buildRoot) + @(Get-ChildItem -LiteralPath $buildRoot -Recurse -Force)
if ($redirected | Where-Object { $_.Attributes -band [IO.FileAttributes]::ReparsePoint }) {
    throw 'target contains a reparse point; inspect it before repairing labels'
}
& icacls.exe $buildRoot /setintegritylevel '(OI)(CI)M' /T /Q
if ($LASTEXITCODE -ne 0) { throw 'Label repair failed. Run this script from elevated Windows PowerShell.' }
& icacls.exe $buildRoot
Write-Host 'Build labels repaired. Rebuild from a normal Windows terminal. Database unchanged.'
