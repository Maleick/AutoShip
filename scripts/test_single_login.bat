@echo off
setlocal enabledelayedexpansion
echo ============================================
echo  Frostreaver - Single Client Test
echo ============================================
echo.

set EQ_PATH=C:\Users\Public\Daybreak Game Company\Installed Games\EverQuest
set DMFT_PATH=C:\Users\xmale\Projects\DMFT
set DMFT_EXE=%DMFT_PATH%\target\release\dmft.exe
set SERVER=Firiona Vie
set ACCOUNT=frostreaver01
set PASSWORD=dr698iDBBa1IpTS

REM Kill any existing EQ
echo [1/5] Killing existing EQ processes...
taskkill /f /im eqgame.exe >nul 2>&1
timeout /t 3 /nobreak >nul

REM Clear DLL logs
del /q "%TEMP%\dmft\dmft-dll.log.*" 2>nul

REM Launch EQ
echo [2/5] Launching EQ (%ACCOUNT%)...
cd /d "%EQ_PATH%"
start "" "%EQ_PATH%\eqgame.exe" patchme /login:%ACCOUNT%
cd /d "%DMFT_PATH%"

REM Wait and find PID
timeout /t 3 /nobreak >nul
set "PID="
for /f "tokens=2" %%P in ('tasklist /fi "imagename eq eqgame.exe" /nh 2^>nul ^| findstr /i "eqgame"') do (
    set "PID=%%P"
)

if not defined PID (
    echo ERROR: eqgame.exe not found!
    pause
    exit /b 1
)

echo   PID: %PID%
echo   Waiting 12s for login screen...
timeout /t 12 /nobreak >nul

REM Inject
echo [3/5] Injecting DLL into PID %PID%...
"%DMFT_EXE%" --inject-pid %PID%
timeout /t 2 /nobreak >nul

REM Login
echo [4/5] Sending login command...
"%DMFT_EXE%" --login-pid %PID% %ACCOUNT% %PASSWORD% "%SERVER%"

echo.
echo [5/5] Monitoring... (60s)
echo   Check log: %TEMP%\dmft\
timeout /t 60 /nobreak >nul

echo.
echo ====== DLL LOG (key events) ======
for /f "delims=" %%f in ('dir /b /od "%TEMP%\dmft\dmft-dll.log.*" 2^>nul') do set LOGFILE=%TEMP%\dmft\%%f
if defined LOGFILE (
    findstr /i "initializing StartLogin PLAY.*clicked Phase EnterWorld in_world error FAILED" "%LOGFILE%"
) else (
    echo No DLL log found.
)
echo ===================================
pause
