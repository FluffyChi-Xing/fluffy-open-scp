$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path $PSScriptRoot -Parent
$outputDirectory = Join-Path $projectRoot 'artifacts'
New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null
$archivePath = Join-Path $outputDirectory ('openscp-website-' + (Get-Date -Format 'yyyyMMdd-HHmmss') + '.zip')
Add-Type -AssemblyName System.IO.Compression
$stream = [System.IO.File]::Open($archivePath, [System.IO.FileMode]::CreateNew)
$archive = [System.IO.Compression.ZipArchive]::new($stream, [System.IO.Compression.ZipArchiveMode]::Create)
try {
    $roots = @('docs', 'scripts', 'tests')
    $files = foreach ($folder in $roots) {
        Get-ChildItem -LiteralPath (Join-Path $projectRoot $folder) -File -Recurse -Force |
            Where-Object { $_.FullName -notmatch '[\\/]\.vitepress[\\/](dist|cache)[\\/]' }
    }
    $files += Get-ChildItem -LiteralPath $projectRoot -File -Force |
        Where-Object { $_.Name -in @('package.json', 'package-lock.json', 'tsconfig.json', 'playwright.config.ts', 'vercel.json', 'README.md', 'AGENTS.md', '.gitignore') }
    foreach ($file in $files) {
        $relativePath = [System.IO.Path]::GetRelativePath($projectRoot, $file.FullName).Replace('\', '/')
        $entry = $archive.CreateEntry($relativePath, [System.IO.Compression.CompressionLevel]::Optimal)
        $inputStream = $file.OpenRead()
        $entryStream = $entry.Open()
        try { $inputStream.CopyTo($entryStream) } finally { $entryStream.Dispose(); $inputStream.Dispose() }
    }
} finally { $archive.Dispose(); $stream.Dispose() }
Write-Output $archivePath
