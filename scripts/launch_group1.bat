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
set ACCOUNTS_FILE=%~dp0accounts_group1.txt
set EXPECTED_CLIENTS=6

if not exist "%ACCOUNTS_FILE%" (
    echo ERROR: Accounts file not found: "%ACCOUNTS_FILE%"
    echo Create it from scripts\accounts_group1.txt.example and add one "username password" per line.
    exit /b 1
)

REM Kill any existing EQ
echo Killing existing EQ processes...
taskkill /f /im eqgame.exe >nul 2>&1
timeout /t 3 /nobreak >nul

REM Clear DLL logs
del /q "%TEMP%\dmft\dmft-dll.log.*" 2>nul

set CLIENT_NUM=0

for /f "usebackq tokens=1,2 eol=#" %%U in ("%ACCOUNTS_FILE%") do (
    if not "%%U"=="" if not "%%V"=="" (
        set /a CLIENT_NUM+=1

        echo.
        echo [!CLIENT_NUM!/%EXPECTED_CLIENTS%] Launching %%U...

        REM Launch EQ
        cd /d "%EQ_PATH%"
        start "" "%EQ_PATH%\eqgame.exe" patchme /login:%%U
        cd /d "%DMFT_PATH%"

        REM Wait for process to start, then find its PID
        timeout /t 3 /nobreak >nul
        set "NEW_PID="
        for /f "tokens=2" %%P in ('tasklist /fi "imagename eq eqgame.exe" /nh 2^>nul ^| findstr /i "eqgame"') do (
            if not defined KNOWN_%%P (
                set "NEW_PID=%%P"
            )
        )

        if not defined NEW_PID (
            echo   ERROR: Could not find new eqgame.exe PID for %%U
        ) else (
            echo   PID: !NEW_PID!
            set "KNOWN_!NEW_PID!=1"

            REM Wait for login screen
            echo   Waiting %INJECT_WAIT%s for login screen...
            timeout /t %INJECT_WAIT% /nobreak >nul

            REM Inject into this specific PID
            echo   Injecting DLL into PID !NEW_PID!...
            "%DMFT_EXE%" --inject-pid !NEW_PID!
            timeout /t %HOOK_WAIT% /nobreak >nul

            REM Send login to this specific PID
            echo   Sending login for %%U...
            "%DMFT_EXE%" --login-pid !NEW_PID! %%U %%V "%SERVER%"
            echo   %%U login sent to PID !NEW_PID!

            REM Stagger before next client
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
echo Log dir: %TEMP%\dmft\
timeout /t 120 /nobreak >nul
echo.
echo Done. Press any key to exit.
pause >nul
