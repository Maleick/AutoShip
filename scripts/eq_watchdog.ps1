# DMFT EQ Watchdog - watches for trigger files, launches EQ + injects + logs in
# Double-click to start, leave running. Claude triggers via SSH.
Write-Host '=== DMFT EQ Watchdog ===' -ForegroundColor Cyan
Write-Host 'Watching for trigger files in C:\Users\xmale\Projects\DMFT\triggers\'
Write-Host 'Press Ctrl+C to stop.'
Write-Host ''

$triggerDir = 'C:\Users\xmale\Projects\DMFT\triggers'
New-Item -ItemType Directory -Force -Path $triggerDir | Out-Null

while ($true) {
    $triggerFile = Get-ChildItem -Path $triggerDir -Filter 'launch_*.txt' -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($triggerFile) {
        $content = Get-Content $triggerFile.FullName
        Write-Host "[$(Get-Date -Format 'HH:mm:ss')] Trigger: $($triggerFile.Name)" -ForegroundColor Green

        $action = $content[0]

        if ($action -eq 'LAUNCH_AND_LOGIN') {
            $account = $content[1]
            $password = $content[2]

            Write-Host "  Launching EQ for $account..."
            $eqPath = 'C:\Users\Public\Daybreak Game Company\Installed Games\EverQuest'
            Start-Process -FilePath "$eqPath\eqgame.exe" -ArgumentList 'patchme' -WorkingDirectory $eqPath

            Write-Host '  Waiting 20s for login screen...'
            Start-Sleep -Seconds 20

            # Inject DLL
            Write-Host '  Injecting DLL...'
            Set-Location 'C:\Users\xmale\Projects\DMFT'
            & .\target\release\dmft.exe --inject 2>&1 | Write-Host

            Start-Sleep -Seconds 5

            # Send login
            Write-Host "  Sending login for $account..."
            & .\target\release\dmft.exe --login $account $password 2>&1 | Write-Host

            Write-Host '  Login chain started!' -ForegroundColor Green
        }
        elseif ($action -eq 'INJECT_ONLY') {
            Set-Location 'C:\Users\xmale\Projects\DMFT'
            & .\target\release\dmft.exe --inject 2>&1 | Write-Host
        }
        elseif ($action -eq 'LOGIN_ONLY') {
            $account = $content[1]
            $password = $content[2]
            Set-Location 'C:\Users\xmale\Projects\DMFT'
            & .\target\release\dmft.exe --login $account $password 2>&1 | Write-Host
        }

        Remove-Item $triggerFile.FullName -Force
        Write-Host "[$(Get-Date -Format 'HH:mm:ss')] Done. Watching..." -ForegroundColor Cyan
    }
    Start-Sleep -Seconds 2
}
