$ErrorActionPreference = "Stop"
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$rootDir = Join-Path $scriptDir ".."
$projects = @("task-channels", "thread-socket")
$iterations = 50

Write-Host "Starting stress test..." -ForegroundColor Cyan

foreach ($project in $projects) {
    $projectDir = Join-Path $rootDir $project
    Write-Host "`nTesting Project: $project" -ForegroundColor Yellow
    Write-Host "Directory: $projectDir"

    Push-Location $projectDir

    # Build first to avoid compilation noise in the loop
    Write-Host "Building release version..."
    cargo build --release
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Build failed for $project"
        Pop-Location
        exit 1
    }

    for ($i = 1; $i -le $iterations; $i++) {
        Write-Host "[$project] Run $i of $iterations ... " -NoNewline

        $sw = [System.Diagnostics.Stopwatch]::StartNew()

        # Run the binary directly using .NET Process for better control
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
        $p.StartInfo.CreateNoWindow = $false

        $p.Start() | Out-Null

        $timeoutSeconds = 15
        $startTime = [System.DateTime]::Now

        while (-not $p.HasExited) {
            if (([System.DateTime]::Now - $startTime).TotalSeconds -gt $timeoutSeconds) {
                $sw.Stop()
                Write-Host "TIMEOUT (>15s)" -ForegroundColor Red
                try { $p.Kill() } catch {}
                Pop-Location
                exit 1
            }
            Start-Sleep -Milliseconds 100
        }

        $sw.Stop()
        $exitCode = $p.ExitCode

        if ($exitCode -eq 0) {
            Write-Host "PASS ($($sw.Elapsed.TotalSeconds.ToString("N2"))s)" -ForegroundColor Green
        } else {
            Write-Host "FAIL (Exit Code: $exitCode)" -ForegroundColor Red
            Pop-Location
            exit 1
        }
    }

    Pop-Location
}

Write-Host "`nAll tests completed successfully!" -ForegroundColor Green
