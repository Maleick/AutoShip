@echo off
echo ============================================
echo  TextQuest - Auto-Login Full Chain
echo ============================================
echo.

REM Configuration
set SERVER=Firiona Vie

REM Credentials must come from environment or prompt (do not hardcode secrets).
if "%TextQuest_ACCOUNT%"=="" (
    set /p ACCOUNT=Enter EQ account: 
) else (
    set ACCOUNT=%TextQuest_ACCOUNT%
)

if "%TextQuest_PASSWORD%"=="" (
    set /p PASSWORD=Enter EQ password: 
) else (
    set PASSWORD=%TextQuest_PASSWORD%
)
set EQ_PATH=C:\Users\Public\Daybreak Game Company\Installed Games\EverQuest
set TextQuest_PATH=C:\Users\xmale\Projects\TextQuest

REM Kill any existing EQ
echo [1/4] Killing existing EQ processes...
taskkill /f /im eqgame.exe >nul 2>&1
timeout /t 3 /nobreak >nul

REM Clear DLL log
del /q "%TEMP%\dmft\textquest-dll.log.*" 2>nul

REM Launch EQ
echo [2/4] Launching EQ (%ACCOUNT%)...
cd /d "%EQ_PATH%"
start "" "%EQ_PATH%\eqgame.exe" patchme /login:%ACCOUNT%
cd /d "%TextQuest_PATH%"
echo Waiting 12s for login screen...
timeout /t 12 /nobreak >nul

REM Inject DLL
echo [3/4] Injecting DLL...
"%TextQuest_PATH%\target\release\textquest.exe" --inject
timeout /t 2 /nobreak >nul

REM Send login
echo [4/4] Sending login command...
"%TextQuest_PATH%\target\release\textquest.exe" --login %ACCOUNT% %PASSWORD% "%SERVER%"
echo.
echo Login chain started! The DLL handles:
echo   - Credential entry + Login click
echo   - PLAY EVERQUEST click
echo   - Character select detection
echo   - Enter World (automatic via Enter key)
echo.
echo Check DLL log: %TEMP%\dmft\textquest-dll.log.*
echo.
echo Monitoring for 60s...
timeout /t 60 /nobreak >nul
echo.
echo ====== RESULTS ======
for /f "delims=" %%f in ('dir /b /od "%TEMP%\dmft\textquest-dll.log.*" 2^>nul') do set LOGFILE=%TEMP%\dmft\%%f
if defined LOGFILE (
    findstr /i "PLAY.*clicked unloaded Sent.Enter Phase" "%LOGFILE%"
) else (
    echo No DLL log found.
)
echo ======================
pause
