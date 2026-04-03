@echo off
setlocal enabledelayedexpansion
echo ============================================
echo  DMFT - Group 1 Launch (6 clients)
echo  Per-PID injection + login targeting
echo ============================================
echo.

set EQ_PATH=C:\Users\Public\Daybreak Game Company\Installed Games\EverQuest
set DMFT_PATH=C:\Users\xmale\Projects\DMFT
set DMFT_EXE=%DMFT_PATH%\target\release\dmft.exe
set SERVER=Firiona Vie
set INJECT_WAIT=12
set HOOK_WAIT=2
set STAGGER=15

REM Kill any existing EQ
echo Killing existing EQ processes...
taskkill /f /im eqgame.exe >nul 2>&1
timeout /t 3 /nobreak >nul

REM Clear DLL logs
del /q "%TEMP%\dmft\dmft-dll.log.*" 2>nul

set CLIENT_NUM=0

REM --- Launch each client by account name only ---
for %%U in (
    frostreaver01
    frostreaver02
    frostreaver03
    frostreaver04
    frostreaver06
    frostreaver07
) do (
    set /a CLIENT_NUM+=1

    echo.
    echo [!CLIENT_NUM!/6] Launching %%U...

    REM Launch EQ
    cd /d "%EQ_PATH%"
    start "" "%EQ_PATH%\eqgame.exe" patchme /login:%%U
    cd /d "%DMFT_PATH%"

    REM Wait for process to start, then find latest PID
    timeout /t 3 /nobreak >nul
    set "NEW_PID="
    for /f "tokens=2" %%P in ('tasklist /fi "imagename eq eqgame.exe" /nh 2^>nul ^| findstr /i "eqgame"') do (
        set "NEW_PID=%%P"
    )

    if not defined NEW_PID (
        echo   ERROR: Could not find eqgame.exe PID for %%U
    ) else (
        echo   PID: !NEW_PID!

        REM Wait for login screen
        echo   Waiting %INJECT_WAIT%s for login screen...
        timeout /t %INJECT_WAIT% /nobreak >nul

        REM Inject into this specific PID
        echo   Injecting DLL into PID !NEW_PID!...
        "%DMFT_EXE%" --inject-pid !NEW_PID!
        timeout /t %HOOK_WAIT% /nobreak >nul

        REM Send login to this specific PID. DMFT prompts for password securely.
        echo   Sending login for %%U...
        "%DMFT_EXE%" login %%U --pid !NEW_PID! --server "%SERVER%"
        echo   %%U login command sent to PID !NEW_PID!

        REM Stagger before next client
        if !CLIENT_NUM! LSS 6 (
            echo   Waiting %STAGGER%s before next client...
            timeout /t %STAGGER% /nobreak >nul
        )
    )
)

echo.
echo ============================================
echo  All 6 clients launched!
echo  Each will auto-login and enter world.
echo ============================================
echo.
echo Monitoring DLL logs for 120s...
echo Log dir: %TEMP%\dmft\
timeout /t 120 /nobreak >nul
echo.
echo Done. Press any key to exit.
pause >nul
