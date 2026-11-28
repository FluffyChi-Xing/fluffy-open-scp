param([string]$Artifact,[string]$Destination='D:/openscp/fluffy-open-scp.exe')
$ErrorActionPreference='Stop'
$source=(Resolve-Path -LiteralPath $Artifact).Path
$destinationPath=[IO.Path]::GetFullPath($Destination)
if($destinationPath -ne 'D:\openscp\fluffy-open-scp.exe'){throw 'Unexpected deployment destination'}
if((Get-Item -LiteralPath $source).Length -lt 1MB){throw 'Invalid desktop artifact'}
$sourceHash=(Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash
$receiptPath="$source.production.json"
if(!(Test-Path -LiteralPath $receiptPath)){throw 'Production build receipt missing; run scripts/build_devtool_desktop.ps1 first'}
$receipt=Get-Content -LiteralPath $receiptPath -Raw|ConvertFrom-Json
if($receipt.customProtocol -ne $true -or $receipt.sha256 -ne $sourceHash){throw 'Production build receipt does not match artifact; rebuild with scripts/build_devtool_desktop.ps1'}
$archive=Join-Path (Split-Path $destinationPath) ('devtool-releases/'+(Get-Date -Format 'yyyyMMdd-HHmmss'))
New-Item -ItemType Directory -Path $archive -Force|Out-Null
Copy-Item -LiteralPath $source -Destination (Join-Path $archive 'new.exe')
$oldHash=$null
if(Test-Path -LiteralPath $destinationPath){
 $oldHash=(Get-FileHash -LiteralPath $destinationPath -Algorithm SHA256).Hash
 Copy-Item -LiteralPath $destinationPath -Destination (Join-Path $archive 'previous.exe')
}
$running=Get-Process -Name fluffy-open-scp -ErrorAction SilentlyContinue|Where-Object {$_.Path -eq $destinationPath}
foreach($process in $running){
 if(!$process.CloseMainWindow()){throw 'Desktop refused graceful close; deployment stopped'}
 if(!$process.WaitForExit(15000)){throw 'Desktop remains open; deployment stopped without forcing exit'}
}
Copy-Item -LiteralPath $source -Destination $destinationPath -Force
$deployedHash=(Get-FileHash -LiteralPath $destinationPath -Algorithm SHA256).Hash
if($deployedHash -ne $sourceHash){throw 'Deployed artifact hash mismatch'}
@{source=$source;sha256=$sourceHash;previousSha256=$oldHash;destination=$destinationPath;time=[DateTimeOffset]::Now.ToString('o')}|ConvertTo-Json|Set-Content (Join-Path $archive 'deployment.json')
Write-Output "Deployed verified desktop; ledger: $archive/deployment.json"
