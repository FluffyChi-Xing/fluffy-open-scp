param(
    [string]$Python = "C:\Users\thunderobot\.cache\codex-runtimes\codex-primary-runtime\dependencies\python\python.exe",
    [string]$Version = ""
)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
if (-not $Version) {
    $gui = Get-Content -LiteralPath (Join-Path $root 'sc5_packager_gui.py') -Raw -Encoding UTF8
    if ($gui -match 'SC5 Asset Packager (\d+\.\d+\.\d+)') { $Version = $Matches[1] }
    else { throw 'Cannot detect release version. Pass -Version x.y.z.' }
}
if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw 'Version must be x.y.z.' }
Push-Location $root
try {
    $release = Join-Path $root (Join-Path 'release' $Version)
    & $Python -m PyInstaller --noconfirm --distpath $release "gui_build.spec"
    if ($LASTEXITCODE -ne 0) { throw "PyInstaller failed: $LASTEXITCODE" }
    Write-Host "EXE: $release\SC5 Asset Packager.exe"
    Write-Host 'EXE only. Portable ZIP generation is disabled per current release preference.'
} finally { Pop-Location }
