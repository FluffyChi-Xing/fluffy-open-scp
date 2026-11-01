# Stage the SC5 Asset Editor exporter into OpenSCP's bundled resources.
#
# Usage:
#   $env:SC5_ADDON_PATH = "C:\path\to\SC5 Asset Editor 1.23.7"
#   pwsh -File scripts/stage-blender-addon.ps1
#
# The script is intentionally explicit: the exporter and its atlas library are
# third-party assets, so redistribution terms must be checked before shipping.

param(
    [string]$Source = $env:SC5_ADDON_PATH,
    [string]$Destination = (Join-Path $PSScriptRoot '..\src-tauri\resources\blender\ny_facade')
)

$ErrorActionPreference = 'Stop'
if (-not $Source) {
    throw 'Set SC5_ADDON_PATH or pass -Source to the SC5 Asset Editor directory.'
}
$Source = (Resolve-Path $Source).Path
$candidate = Join-Path $Source 'ny_facade'
if (-not (Test-Path $candidate)) {
    if (Test-Path (Join-Path $Source '__init__.py')) {
        $candidate = $Source
    } else {
        throw "No ny_facade package found under $Source"
    }
}
if (-not (Test-Path (Join-Path $candidate '__init__.py'))) {
    throw "Not an SC5 Asset Editor package: $candidate"
}

$Destination = [System.IO.Path]::GetFullPath($Destination)
if (Test-Path $Destination) {
    Remove-Item $Destination -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $Destination | Out-Null
Copy-Item -Path (Join-Path $candidate '*') -Destination $Destination -Recurse -Force

$files = Get-ChildItem $Destination -Recurse -File
$bytes = ($files | Measure-Object Length -Sum).Sum
Write-Host ("Staged {0} files, {1:N1} MiB -> {2}" -f $files.Count, ($bytes / 1MB), $Destination)
