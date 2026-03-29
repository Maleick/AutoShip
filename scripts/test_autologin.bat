@echo off
echo ============================================
echo  Frostreaver - Auto-Login Full Chain Test
echo ============================================
echo.
echo Step 1: Launching EQ (frostreaver01) with /login flag...
cd /d "C:\Users\Public\Daybreak Game Company\Installed Games\EverQuest"
start "" "C:\Users\Public\Daybreak Game Company\Installed Games\EverQuest\eqgame.exe" patchme /login:frostreaver01
echo.
echo Step 2: Waiting 20 seconds for login screen to load...
timeout /t 20 /nobreak
echo.
echo Step 3: Injecting DLL...
cd /d C:\Users\xmale\Projects\DMFT
target\release\dmft.exe --inject
echo.
echo Step 4: Waiting 5 seconds for DLL hooks...
timeout /t 5 /nobreak
echo.
echo Step 5: Sending login command (password via DLL)...
target\release\dmft.exe --login frostreaver01 dr698iDBBa1IpTS
echo.
echo Step 6: Monitoring DLL log for 30 seconds...
timeout /t 30 /nobreak
echo.
echo ====== LOGIN RESULTS ======
for /f "delims=" %%f in ('dir /b /od "%TEMP%\dmft\dmft-dll.log.*" 2^>nul') do set LOGFILE=%TEMP%\dmft\%%f
if defined LOGFILE (
    echo Log file: %LOGFILE%
    findstr /i "transition credential Login.*screen PLAY.*EVERQUEST server.*select character Inline" "%LOGFILE%"
) else (
    echo No DLL log file found.
)
echo =================================
echo.
echo Done. Press any key to exit.
pause >nul
