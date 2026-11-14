param(
    [Parameter(Mandatory)]
    [string]$Source
)

$ErrorActionPreference = 'Stop'
$sourceRoot = (Resolve-Path -LiteralPath $Source).Path
$catalogPath = Join-Path $sourceRoot 'catalog.json'
$catalog = Get-Content -LiteralPath $catalogPath -Raw -Encoding utf8 | ConvertFrom-Json
$packs = @($catalog.packs.PSObject.Properties.Name)
if ($packs.Count -eq 0) {
    throw 'The atlas catalog contains no packs.'
}

# Check the full input before writing anything to the bundle directory.
$images = foreach ($pack in $packs) {
    if ($pack -notmatch '^[A-Za-z0-9_-]+$') {
        throw "Invalid atlas pack name: $pack"
    }
    foreach ($slot in 1..5) {
        $relative = Join-Path $pack "slot$slot.png"
        if (-not (Test-Path -LiteralPath (Join-Path $sourceRoot $relative) -PathType Leaf)) {
            throw "Missing atlas image: $relative"
        }
        $relative
    }
}

$destination = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../src-tauri/resources/blend-atlas'))
New-Item -ItemType Directory -Path $destination -Force | Out-Null
foreach ($relative in $images) {
    $target = Join-Path $destination $relative
    New-Item -ItemType Directory -Path (Split-Path -Parent $target) -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $sourceRoot $relative) -Destination $target -Force
}
Copy-Item -LiteralPath $catalogPath -Destination (Join-Path $destination 'catalog.json') -Force
Write-Output "Staged $($packs.Count) atlas packs in $destination."
