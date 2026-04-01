@echo off
echo ============================================
echo  DMFT - Auto-Login Calibration Test
echo ============================================
echo.
echo Step 1: Launching EQ (frostreaver01)...
cd /d "C:\Users\Public\Daybreak Game Company\Installed Games\EverQuest"
start "" "C:\Users\Public\Daybreak Game Company\Installed Games\EverQuest\eqgame.exe" patchme /login:frostreaver01
echo.
echo Step 2: Wait for the PASSWORD screen to appear.
echo          DO NOT type the password.
echo          Press any key here when you see the password field...
pause >nul
echo.
echo Step 3: Injecting DLL...
cd /d C:\Users\xmale\Projects\DMFT
target\release\dmft.exe --inject
echo.
echo Step 4: Waiting 5 seconds for DLL hooks...
timeout /t 5 /nobreak >nul
echo.
echo Step 5: Running calibration...
target\release\dmft.exe --calibrate
echo.
echo Step 6: Checking DLL log...
timeout /t 2 /nobreak >nul
echo.
echo ====== CALIBRATION RESULTS ======
for /f "delims=" %%f in ('dir /b /od "%TEMP%\dmft\dmft-dll.log.*" 2^>nul') do set LOGFILE=%TEMP%\dmft\%%f
if defined LOGFILE (
    echo Log file: %LOGFILE%
    findstr /i "calibrat login eqmain LoginClient EQLogin HWND CalibrateLogin" "%LOGFILE%"
) else (
    echo No DLL log file found.
)
echo =================================
echo.
echo Done. Press any key to exit.
pause >nul
