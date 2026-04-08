@echo off
setlocal enabledelayedexpansion
echo ============================================
echo  TextQuest - Group 1 Launch (6 clients)
echo  Per-PID injection + login targeting
echo ============================================
echo.

set EQ_PATH=C:\Users\Public\Daybreak Game Company\Installed Games\EverQuest
set TextQuest_PATH=C:\Users\xmale\Projects\TextQuest
set TextQuest_EXE=%TextQuest_PATH%\target\release\textquest.exe
set SERVER=Firiona Vie
set INJECT_WAIT=12
set HOOK_WAIT=2
set STAGGER=15
set ACCOUNTS_FILE=%~dp0accounts_group1.txt
set EXPECTED_CLIENTS=6

REM Account list: name password
set ACCT1=frostreaver01 dr698iDBBa1IpTS
set ACCT2=frostreaver02 rLlkT9TEzVzbtAJ
set ACCT3=frostreaver03 2U2dDrgMuI6sDTi
set ACCT4=frostreaver04 67FbF2LmZMEFIR7
set ACCT5=frostreaver06 DXOXKC1dIvSFXDB
if not defined ACCT6_PASS (
    echo ERROR: ACCT6_PASS environment variable is not set.
    echo Set ACCT6_PASS before running this script.
    exit /b 1
)
set ACCT6=frostreaver07 %ACCT6_PASS%

REM Kill any existing EQ
echo Killing existing EQ processes...
taskkill /f /im eqgame.exe >nul 2>&1
timeout /t 3 /nobreak >nul

REM Clear DLL logs
del /q "%TEMP%\textquest\textquest-dll.log.*" 2>nul

set CLIENT_NUM=0

REM --- Launch function ---
REM Uses: ACCT (name password), CLIENT_NUM, captures PID
for %%A in (
    "frostreaver01 dr698iDBBa1IpTS"
    "frostreaver02 rLlkT9TEzVzbtAJ"
    "frostreaver03 2U2dDrgMuI6sDTi"
    "frostreaver04 67FbF2LmZMEFIR7"
    "frostreaver06 DXOXKC1dIvSFXDB"
    "!ACCT6!"
) do (
    set /a CLIENT_NUM+=1

        REM Launch EQ
        cd /d "%EQ_PATH%"
        start "" "%EQ_PATH%\eqgame.exe" patchme /login:%%U
        cd /d "%TextQuest_PATH%"

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
            "%TextQuest_EXE%" --inject-pid !NEW_PID!
            timeout /t %HOOK_WAIT% /nobreak >nul

            REM Send login to this specific PID
            echo   Sending login for %%U...
            "%TextQuest_EXE%" --login-pid !NEW_PID! %%U %%V "%SERVER%"
            echo   %%U login sent to PID !NEW_PID!

        REM Stagger before next client
        if !CLIENT_NUM! LSS 6 (
            echo   Waiting %STAGGER%s before next client...
            timeout /t %STAGGER% /nobreak >nul
        )
    )
)

if %CLIENT_NUM% EQU 0 (
    echo ERROR: No valid account entries found in "%ACCOUNTS_FILE%".
    exit /b 1
)

echo.
echo ============================================
echo  %CLIENT_NUM% clients launched!
echo  Each will auto-login and enter world.
echo ============================================
echo.
echo Monitoring DLL logs for 120s...
echo Log dir: %TEMP%\textquest\
timeout /t 120 /nobreak >nul
echo.
echo Done. Press any key to exit.
pause >nul
