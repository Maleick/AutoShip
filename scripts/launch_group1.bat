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

REM Account list: name password
set ACCT1=frostreaver01 dr698iDBBa1IpTS
set ACCT2=frostreaver02 rLlkT9TEzVzbtAJ
set ACCT3=frostreaver03 2U2dDrgMuI6sDTi
set ACCT4=frostreaver04 67FbF2LmZMEFIR7
set ACCT5=frostreaver06 DXOXKC1dIvSFXDB
set ACCT6=frostreaver07 aTWmNmNn4jYAXYf

REM Kill any existing EQ
echo Killing existing EQ processes...
taskkill /f /im eqgame.exe >nul 2>&1
timeout /t 3 /nobreak >nul

REM Clear DLL logs
del /q "%TEMP%\dmft\dmft-dll.log.*" 2>nul

REM Snapshot existing PIDs before first launch
for /f "tokens=2" %%a in ('tasklist /fi "imagename eq eqgame.exe" /nh 2^>nul ^| findstr /i "eqgame"') do (
    set "EXISTING_%%a=1"
)

set CLIENT_NUM=0

REM --- Launch function ---
REM Uses: ACCT (name password), CLIENT_NUM, captures PID
for %%A in (
    "frostreaver01 dr698iDBBa1IpTS"
    "frostreaver02 rLlkT9TEzVzbtAJ"
    "frostreaver03 2U2dDrgMuI6sDTi"
    "frostreaver04 67FbF2LmZMEFIR7"
    "frostreaver06 DXOXKC1dIvSFXDB"
    "frostreaver07 aTWmNmNn4jYAXYf"
) do (
    set /a CLIENT_NUM+=1
    for /f "tokens=1,2" %%U in (%%A) do (
        echo.
        echo [!CLIENT_NUM!/6] Launching %%U...

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
            if !CLIENT_NUM! LSS 6 (
                echo   Waiting %STAGGER%s before next client...
                timeout /t %STAGGER% /nobreak >nul
            )
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
