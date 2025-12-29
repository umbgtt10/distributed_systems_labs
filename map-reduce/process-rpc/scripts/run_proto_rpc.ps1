# run_proto_rpc.ps1

Write-Host "=== MapReduce Process-RPC Launcher ===" -ForegroundColor Cyan

# 1. Cleanup pending processes
Write-Host "`n[1/3] Cleaning up pending processes..." -ForegroundColor Yellow
Stop-Process -Name "process-rpc" -Force -ErrorAction SilentlyContinue
Stop-Process -Name "map-reduce-process-rpc" -Force -ErrorAction SilentlyContinue
Write-Host "Cleanup complete." -ForegroundColor Green

# 2. Check/Install Protobuf
Write-Host "`n[2/3] Checking for Protocol Buffers compiler (protoc)..." -ForegroundColor Yellow
if (-not (Get-Command protoc -ErrorAction SilentlyContinue)) {
    # Check common Winget location first to avoid unnecessary install
    $protocExe = Get-ChildItem "$env:LOCALAPPDATA\Microsoft\WinGet\Packages" -Recurse -Filter "protoc.exe" -ErrorAction SilentlyContinue | Select-Object -First 1

    if ($protocExe) {
        Write-Host "Found protoc at $($protocExe.DirectoryName). Adding to PATH..."
        $env:Path += ";$($protocExe.DirectoryName)"
    } else {
        Write-Host "protoc not found. Installing via Winget..."
        winget install protobuf --accept-source-agreements --accept-package-agreements

        # Try to find it again after install
        $protocExe = Get-ChildItem "$env:LOCALAPPDATA\Microsoft\WinGet\Packages" -Recurse -Filter "protoc.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($protocExe) {
            $env:Path += ";$($protocExe.DirectoryName)"
            Write-Host "Protobuf installed and added to PATH." -ForegroundColor Green
        } else {
            Write-Error "Failed to find protoc after installation. Please install Protocol Buffers manually or restart your terminal."
            exit 1
        }
    }
} else {
    Write-Host "Protoc is already in PATH." -ForegroundColor Green
}

# 3. Start the application
Write-Host "`n[3/3] Starting process-rpc..." -ForegroundColor Yellow
$scriptPath = $PSScriptRoot
# Assuming script is in map-reduce/process-rpc/scripts/
$projectDir = Join-Path $scriptPath ".."
$projectDir = Resolve-Path $projectDir

if (Test-Path $projectDir) {
    Push-Location $projectDir
    Write-Host "Working directory: $projectDir"
    try {
        # Run directly from the directory so it finds config.json
        cargo run --release
    } catch {
        Write-Error "Failed to run application: $_"
    } finally {
        Pop-Location
    }
} else {
    Write-Error "Could not find project directory at $projectDir"
    exit 1
}
