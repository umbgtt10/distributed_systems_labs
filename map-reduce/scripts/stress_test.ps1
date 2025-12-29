$ErrorActionPreference = "Stop"
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$rootDir = Join-Path $scriptDir ".."
$projects = @("task-channels", "thread-socket", "process-rpc")
$iterations = 5

Write-Host "Starting stress test..." -ForegroundColor Cyan

# 1. Build all projects first
foreach ($project in $projects) {
    # Kill any lingering processes that might lock the executable
    $exeName = "map-reduce-$project"
    Stop-Process -Name $exeName -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 200 # Give OS time to release file locks

    $projectDir = Join-Path $rootDir $project
    Write-Host "`nBuilding Project: $project" -ForegroundColor Yellow

    Push-Location $projectDir
    Write-Host "Building release version..."
    cargo build --release --quiet
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Build failed for $project"
        Pop-Location
        exit 1
    }
    Pop-Location
}

# 2. Run Stress Tests
foreach ($project in $projects) {
    $projectDir = Join-Path $rootDir $project
    Write-Host "`nTesting Project: $project" -ForegroundColor Yellow

    Push-Location $projectDir

    for ($i = 1; $i -le $iterations; $i++) {
        Write-Host "[$project] Run $i of $iterations ... " -NoNewline

        $sw = [System.Diagnostics.Stopwatch]::StartNew()

        $binaryName = "map-reduce-$project.exe"
        $binaryPath = Join-Path $rootDir "target\release\$binaryName"

        if (-not (Test-Path $binaryPath)) {
             Write-Error "Binary not found at $binaryPath"
             Pop-Location
             exit 1
        }

        $p = New-Object System.Diagnostics.Process
        $p.StartInfo.FileName = $binaryPath
        $p.StartInfo.WorkingDirectory = $projectDir
        $p.StartInfo.UseShellExecute = $false
        $p.StartInfo.CreateNoWindow = $true
        $p.StartInfo.RedirectStandardOutput = $true
        $p.StartInfo.RedirectStandardError = $true

        $p.Start() | Out-Null

        # Use async readers to prevent deadlock
        $outputReader = $p.StandardOutput
        $errorReader = $p.StandardError

        $outputTask = $outputReader.ReadToEndAsync()
        $errorTask = $errorReader.ReadToEndAsync()

        $timeoutSeconds = 30
        $startTime = [System.DateTime]::Now

        while (-not $p.HasExited) {
            if (([System.DateTime]::Now - $startTime).TotalSeconds -gt $timeoutSeconds) {
                $sw.Stop()
                Write-Host "TIMEOUT (>30s)" -ForegroundColor Red
                try {
                    $p.Kill()
                    $p.WaitForExit(1000)
                } catch {}

                # Try to get partial output
                try {
                    $output = if ($outputTask.IsCompleted) { $outputTask.Result } else { "" }
                    $err = if ($errorTask.IsCompleted) { $errorTask.Result } else { "" }
                    Write-Host "--- STDOUT ---" -ForegroundColor Gray
                    Write-Host $output
                    Write-Host "--- STDERR ---" -ForegroundColor Gray
                    Write-Host $err
                } catch {}

                Pop-Location
                exit 1
            }
            Start-Sleep -Milliseconds 100
        }

        $p.WaitForExit()
        $sw.Stop()
        $exitCode = $p.ExitCode

        # Wait for async reads to complete
        $output = $outputTask.Result
        $err = $errorTask.Result

        if ($exitCode -eq 0) {
            Write-Host "PASS ($($sw.Elapsed.TotalSeconds.ToString("N2"))s)" -ForegroundColor Green
        } else {
            Write-Host "FAIL (Exit Code: $exitCode)" -ForegroundColor Red
            Write-Host "--- STDOUT ---" -ForegroundColor Gray
            Write-Host $output
            Write-Host "--- STDERR ---" -ForegroundColor Gray
            Write-Host $err
            Pop-Location
            exit 1
        }
    }

    Pop-Location
}

Write-Host "`nAll tests completed successfully!" -ForegroundColor Green
