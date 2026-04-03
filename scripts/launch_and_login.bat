@echo off
echo ============================================
echo  DMFT - Auto-Login Full Chain
echo ============================================
echo.

REM Configuration
if "%DMFT_ACCOUNT%"=="" (set ACCOUNT=<SET_DMFT_ACCOUNT>) else (set ACCOUNT=%DMFT_ACCOUNT%)
if "%DMFT_PASSWORD%"=="" (set PASSWORD=<SET_DMFT_PASSWORD>) else (set PASSWORD=%DMFT_PASSWORD%)
set SERVER=Firiona Vie
set EQ_PATH=C:\Users\Public\Daybreak Game Company\Installed Games\EverQuest
set DMFT_PATH=C:\Users\xmale\Projects\DMFT

REM Kill any existing EQ
echo [1/4] Killing existing EQ processes...
taskkill /f /im eqgame.exe >nul 2>&1
timeout /t 3 /nobreak >nul

REM Clear DLL log
del /q "%TEMP%\dmft\dmft-dll.log.*" 2>nul

REM Launch EQ
echo [2/4] Launching EQ (%ACCOUNT%)...
cd /d "%EQ_PATH%"
start "" "%EQ_PATH%\eqgame.exe" patchme /login:%ACCOUNT%
cd /d "%DMFT_PATH%"
echo Waiting 12s for login screen...
timeout /t 12 /nobreak >nul

REM Inject DLL
echo [3/4] Injecting DLL...
"%DMFT_PATH%\target\release\dmft.exe" --inject
timeout /t 2 /nobreak >nul

REM Send login
echo [4/4] Sending login command...
"%DMFT_PATH%\target\release\dmft.exe" --login %ACCOUNT% %PASSWORD% "%SERVER%"
echo.
echo Login chain started! The DLL handles:
echo   - Credential entry + Login click
echo   - PLAY EVERQUEST click
echo   - Character select detection
echo   - Enter World (automatic via Enter key)
echo.
echo Check DLL log: %TEMP%\dmft\dmft-dll.log.*
echo.
echo Monitoring for 60s...
timeout /t 60 /nobreak >nul
echo.
echo ====== RESULTS ======
for /f "delims=" %%f in ('dir /b /od "%TEMP%\dmft\dmft-dll.log.*" 2^>nul') do set LOGFILE=%TEMP%\dmft\%%f
if defined LOGFILE (
    findstr /i "PLAY.*clicked unloaded Sent.Enter Phase" "%LOGFILE%"
) else (
    echo No DLL log found.
)
echo ======================
pause
