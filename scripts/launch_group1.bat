@echo off
echo ============================================
echo  Frostreaver - Group 1 Launch (5 clients)
echo ============================================
echo.

set EQ_PATH=C:\Users\Public\Daybreak Game Company\Installed Games\EverQuest
set DMFT_PATH=C:\Users\xmale\Projects\DMFT
set SERVER=Firiona Vie
set STAGGER=15

REM Kill any existing EQ
echo Killing existing EQ processes...
taskkill /f /im eqgame.exe >nul 2>&1
timeout /t 3 /nobreak >nul

REM Clear DLL logs
del /q "%TEMP%\dmft\dmft-dll.log.*" 2>nul

REM --- Client 1: frostreaver01 (WAR) ---
echo.
echo [1/5] Launching frostreaver01 (WAR)...
cd /d "%EQ_PATH%"
start "" "%EQ_PATH%\eqgame.exe" patchme /login:frostreaver01
cd /d "%DMFT_PATH%"
timeout /t 12 /nobreak >nul
"%DMFT_PATH%\target\release\dmft.exe" --inject
timeout /t 2 /nobreak >nul
"%DMFT_PATH%\target\release\dmft.exe" --login frostreaver01 dr698iDBBa1IpTS "%SERVER%"
echo   frostreaver01 login sent
timeout /t %STAGGER% /nobreak >nul

REM --- Client 2: frostreaver02 (SHM) ---
echo [2/5] Launching frostreaver02 (SHM)...
cd /d "%EQ_PATH%"
start "" "%EQ_PATH%\eqgame.exe" patchme /login:frostreaver02
cd /d "%DMFT_PATH%"
timeout /t 12 /nobreak >nul
"%DMFT_PATH%\target\release\dmft.exe" --inject
timeout /t 2 /nobreak >nul
"%DMFT_PATH%\target\release\dmft.exe" --login frostreaver02 rLlkT9TEzVzbtAJ "%SERVER%"
echo   frostreaver02 login sent
timeout /t %STAGGER% /nobreak >nul

REM --- Client 3: frostreaver03 (CLR) ---
echo [3/5] Launching frostreaver03 (CLR)...
cd /d "%EQ_PATH%"
start "" "%EQ_PATH%\eqgame.exe" patchme /login:frostreaver03
cd /d "%DMFT_PATH%"
timeout /t 12 /nobreak >nul
"%DMFT_PATH%\target\release\dmft.exe" --inject
timeout /t 2 /nobreak >nul
"%DMFT_PATH%\target\release\dmft.exe" --login frostreaver03 2U2dDrgMuI6sDTi "%SERVER%"
echo   frostreaver03 login sent
timeout /t %STAGGER% /nobreak >nul

REM --- Client 4: frostreaver04 (CLR) ---
echo [4/5] Launching frostreaver04 (CLR)...
cd /d "%EQ_PATH%"
start "" "%EQ_PATH%\eqgame.exe" patchme /login:frostreaver04
cd /d "%DMFT_PATH%"
timeout /t 12 /nobreak >nul
"%DMFT_PATH%\target\release\dmft.exe" --inject
timeout /t 2 /nobreak >nul
"%DMFT_PATH%\target\release\dmft.exe" --login frostreaver04 67FbF2LmZMEFIR7 "%SERVER%"
echo   frostreaver04 login sent
timeout /t %STAGGER% /nobreak >nul

REM --- Client 5: frostreaver06 (BRD) --- (skipping 05, not created)
echo [5/5] Launching frostreaver06 (BRD)...
cd /d "%EQ_PATH%"
start "" "%EQ_PATH%\eqgame.exe" patchme /login:frostreaver06
cd /d "%DMFT_PATH%"
timeout /t 12 /nobreak >nul
"%DMFT_PATH%\target\release\dmft.exe" --inject
timeout /t 2 /nobreak >nul
"%DMFT_PATH%\target\release\dmft.exe" --login frostreaver06 DXOXKC1dIvSFXDB "%SERVER%"
echo   frostreaver06 login sent

echo.
echo ============================================
echo  All 5 clients launched!
echo  Each will auto-login and enter world.
echo ============================================
echo.
echo Monitoring for 120s...
timeout /t 120 /nobreak >nul
echo.
echo Done. Press any key to exit.
pause >nul
