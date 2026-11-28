# Build the devtool desktop with embedded production assets.
# Direct cargo --release without tauri/custom-protocol still loads devUrl.
$ErrorActionPreference='Stop'
npm run build
if($LASTEXITCODE -ne 0){throw 'Frontend production build failed'}
cargo build -p fluffy-open-scp --release --bin fluffy-open-scp --features tauri/custom-protocol
if($LASTEXITCODE -ne 0){throw 'Desktop production build failed'}
