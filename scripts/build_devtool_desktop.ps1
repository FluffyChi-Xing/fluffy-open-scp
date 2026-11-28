# Build the devtool desktop with embedded production assets.
# Direct cargo --release without tauri/custom-protocol still loads devUrl.
$ErrorActionPreference='Stop'
npm run build
if($LASTEXITCODE -ne 0){throw 'Frontend production build failed'}
cargo build -p fluffy-open-scp --release --bin fluffy-open-scp --features tauri/custom-protocol
if($LASTEXITCODE -ne 0){throw 'Desktop production build failed'}
$artifact=Join-Path $(if($env:CARGO_TARGET_DIR){$env:CARGO_TARGET_DIR}else{'target'}) 'release/fluffy-open-scp.exe'
@{sha256=(Get-FileHash -LiteralPath $artifact -Algorithm SHA256).Hash;customProtocol=$true;time=[DateTimeOffset]::Now.ToString('o')}|ConvertTo-Json|Set-Content -LiteralPath "$artifact.production.json"
