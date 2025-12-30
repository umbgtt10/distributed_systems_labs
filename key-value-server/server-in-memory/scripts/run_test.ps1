#!/usr/bin/env pwsh
# Script to run a 30-second stress test with the KV server and client

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RootDir = Split-Path -Parent $ScriptDir
$ProjectRoot = Split-Path -Parent $RootDir

Write-Host "=== KV Server Stress Test ===" -ForegroundColor Cyan
Write-Host ""

# Kill any leftover processes
Write-Host "Cleaning up any leftover processes..." -ForegroundColor Yellow
Get-Process | Where-Object { $_.ProcessName -like "*key-value-server*" } | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 500

# Build both server and client
Write-Host "Building server..." -ForegroundColor Yellow
Push-Location $ProjectRoot
cargo build --release --bin key-value-server-in-memory
if ($LASTEXITCODE -ne 0) {
    Pop-Location
    Write-Host "Build failed!" -ForegroundColor Red
    exit 1
}
Pop-Location

# Start the server in background (server spawns its own client)
Write-Host ""
Write-Host "Starting KV server with embedded client..." -ForegroundColor Green
$ServerProcess = Start-Process -FilePath "$ProjectRoot\target\release\key-value-server-in-memory.exe" -PassThru -NoNewWindow
Start-Sleep -Seconds 2

if ($ServerProcess.HasExited) {
    Write-Host "Server failed to start!" -ForegroundColor Red
    exit 1
}

Write-Host "Server started (PID: $($ServerProcess.Id))" -ForegroundColor Green

# Run for test duration + 2 seconds buffer
Write-Host ""
Write-Host "Running stress test (will auto-shutdown after config duration)..." -ForegroundColor Cyan
Start-Sleep -Seconds 32

Write-Host ""
Write-Host "Test completed. Waiting for graceful shutdown..." -ForegroundColor Yellow
Start-Sleep -Seconds 2

# Server should have already shut down gracefully
if (!$ServerProcess.HasExited) {
    Write-Host "Server still running, forcing shutdown (PID: $($ServerProcess.Id))..." -ForegroundColor Yellow
    Stop-Process -Id $ServerProcess.Id -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 500
} else {
    Write-Host "Server shut down gracefully" -ForegroundColor Green
}

Write-Host ""
Write-Host "=== Test Complete ===" -ForegroundColor Green
Write-Host ""
