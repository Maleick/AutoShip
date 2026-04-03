## DMFT - Group 1 Launch (6 clients) with per-PID targeting
## Requires Console session with GPU (not RDP)

$EqPath = "C:\Users\Public\Daybreak Game Company\Installed Games\EverQuest"
$DmftExe = "C:\Users\xmale\Projects\DMFT\target\release\dmft.exe"
$Server = "Firiona Vie"
$InjectWait = 12
$HookWait = 2
$Stagger = 15

$Accounts = @(
    @{ Name = "frostreaver01"; Pass = "<ACCOUNT1_PASSWORD>" },
    @{ Name = "frostreaver02"; Pass = "<ACCOUNT2_PASSWORD>" },
    @{ Name = "frostreaver03"; Pass = "<ACCOUNT3_PASSWORD>" },
    @{ Name = "frostreaver04"; Pass = "<ACCOUNT4_PASSWORD>" },
    @{ Name = "frostreaver06"; Pass = "<ACCOUNT5_PASSWORD>" },
    @{ Name = "frostreaver07"; Pass = "<ACCOUNT6_PASSWORD>" }
)

Write-Host "============================================"
Write-Host " DMFT - Group 1 Launch (6 clients)"
Write-Host "============================================"
Write-Host ""

# Kill existing EQ
Write-Host "Killing existing EQ processes..."
Stop-Process -Name eqgame -Force -ErrorAction SilentlyContinue
Start-Sleep -Seconds 3

# Clear DLL logs
Remove-Item "$env:TEMP\dmft\dmft-dll.log.*" -Force -ErrorAction SilentlyContinue

$Results = @()

for ($i = 0; $i -lt $Accounts.Count; $i++) {
    $acct = $Accounts[$i]
    $num = $i + 1
    Write-Host ""
    Write-Host "[$num/6] Launching $($acct.Name)..."

    # Launch EQ
    $proc = Start-Process -FilePath "$EqPath\eqgame.exe" `
        -ArgumentList "patchme", "/login:$($acct.Name)" `
        -WorkingDirectory $EqPath `
        -PassThru

    $procId = $proc.Id
    Write-Host "  PID: $procId"

    # Wait for login screen
    Write-Host "  Waiting ${InjectWait}s for login screen..."
    Start-Sleep -Seconds $InjectWait

    # Inject
    Write-Host "  Injecting DLL..."
    & $DmftExe --inject-pid $procId
    Start-Sleep -Seconds $HookWait

    # Login
    Write-Host "  Sending login..."
    & $DmftExe --login-pid $procId $acct.Name $acct.Pass $Server
    Write-Host "  $($acct.Name) login sent to PID $procId"

    $Results += [PSCustomObject]@{
        Account = $acct.Name
        PID = $procId
    }

    # Stagger (skip after last client)
    if ($i -lt ($Accounts.Count - 1)) {
        Write-Host "  Waiting ${Stagger}s before next client..."
        Start-Sleep -Seconds $Stagger
    }
}

Write-Host ""
Write-Host "============================================"
Write-Host " All 6 clients launched!"
Write-Host "============================================"
Write-Host ""
Write-Host "Account PIDs:"
$Results | Format-Table -AutoSize

# Monitor
Write-Host "Monitoring for 120s... (check $env:TEMP\dmft\ for logs)"
Start-Sleep -Seconds 30

Write-Host ""
Write-Host "====== Status Check ======"
foreach ($r in $Results) {
    $proc = Get-Process -Id $r.PID -ErrorAction SilentlyContinue
    if ($proc) {
        $memMB = [math]::Round($proc.WorkingSet64 / 1MB)
        $status = if ($memMB -gt 500) { "IN WORLD" } else { "loading/char select" }
        Write-Host "  $($r.Account) (PID $($r.PID)): ${memMB}MB - $status"
    } else {
        Write-Host "  $($r.Account) (PID $($r.PID)): CRASHED/EXITED"
    }
}
Write-Host "=========================="
Write-Host ""
Write-Host "Press any key to exit..."
$null = $Host.UI.RawUI.ReadKey("NoEcho,IncludeKeyDown")
