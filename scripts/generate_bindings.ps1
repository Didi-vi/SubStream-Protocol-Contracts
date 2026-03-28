# Generate TypeScript bindings for the SubStream Soroban contract (PowerShell)

$CONTRACT_WASM = "./target/wasm32-unknown-unknown/release/substream_contracts.wasm"
$OUTPUT_DIR = "./bindings/substream"

# 1. Build contract if wasm doesn't exist
if (-not (Test-Path $CONTRACT_WASM)) {
    Write-Host "WASM not found. Building contract..." -ForegroundColor Yellow
    cargo build --target wasm32-unknown-unknown --release
}

# 2. Check for stellar-cli
if (-not (Get-Command "stellar" -ErrorAction SilentlyContinue)) {
    Write-Host "stellar-cli could not be found. Please install it with 'cargo install --locked stellar-cli'" -ForegroundColor Red
    exit
}

# 3. Generate bindings
Write-Host "Generating TypeScript bindings for $CONTRACT_WASM..." -ForegroundColor Cyan
if (-not (Test-Path $OUTPUT_DIR)) {
    New-Item -ItemType Directory -Path $OUTPUT_DIR -Force | Out-Null
}

stellar contract bindings typescript --wasm $CONTRACT_WASM --output-dir $OUTPUT_DIR --overwrite

Write-Host "Bindings generated in $OUTPUT_DIR" -ForegroundColor Green
