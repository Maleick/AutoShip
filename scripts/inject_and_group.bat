@echo off
REM ============================================================
REM  Inject DLL into all EQ clients and list PIDs for grouping
REM  Run AFTER all 6 clients are logged in with characters
REM ============================================================
setlocal enabledelayedexpansion

echo.
echo  === DMFT Inject ^& Group ===
echo.

REM --- Locate dmft.exe ---
set "DMFT_EXE=%~dp0..\target\release\dmft.exe"
if not exist "%DMFT_EXE%" (
    set "DMFT_EXE=%~dp0..\target\debug\dmft.exe"
)
if not exist "%DMFT_EXE%" (
    echo [ERROR] Cannot find dmft.exe. Run: cargo build --release
    goto :done
)

REM --- Find all eqgame.exe PIDs ---
set COUNT=0
for /f "tokens=2" %%p in ('tasklist /fi "imagename eq eqgame.exe" /fo list ^| findstr "PID:"') do (
    set /a COUNT+=1
    set "PID_!COUNT!=%%p"
)

if %COUNT%==0 (
    echo [ERROR] No eqgame.exe processes found. Launch EQ first.
    goto :done
)
echo  Found %COUNT% EQ client(s).
echo.

REM --- Inject into all clients ---
echo  Injecting DLL into all clients...
"%DMFT_EXE%" --inject
if %ERRORLEVEL% neq 0 (
    echo [WARN] Injection returned error code %ERRORLEVEL%.
)
echo.

REM --- Wait for hooks to initialize ---
echo  Waiting 3 seconds for hooks to initialize...
timeout /t 3 /nobreak >nul
echo.

REM --- List PIDs for manual grouping ---
echo  === Injected Clients ===
echo.
for /l %%i in (1,1,%COUNT%) do (
    echo   Client %%i: PID=!PID_%%i!
)
echo.

REM --- Send /invite from every client to every other ---
REM EQ ignores /invite when already grouped, so brute-force is safe.
REM However, /invite requires character names, not PIDs.
REM
REM For now: use cmd_all.bat to send group invites manually:
REM   cmd_all.bat "/invite Charname"
REM
REM Or send commands to individual PIDs:
REM   dmft.exe --cmd <PID> "/invite Charname"

echo  === Grouping Instructions ===
echo.
echo  Option A - Leader invites each member by name:
echo    dmft.exe --cmd ^<leader_pid^> "/invite Membername"
echo.
echo  Option B - Broadcast invite acceptance from all:
echo    cmd_all.bat "/invite"
echo.
echo  Option C - Full auto (fill in character names below):
echo    REM Uncomment and edit these lines with actual character names:
echo    REM "%DMFT_EXE%" --cmd !PID_1! "/invite Char2"
echo    REM "%DMFT_EXE%" --cmd !PID_1! "/invite Char3"
echo    REM "%DMFT_EXE%" --cmd !PID_1! "/invite Char4"
echo    REM "%DMFT_EXE%" --cmd !PID_1! "/invite Char5"
echo    REM "%DMFT_EXE%" --cmd !PID_1! "/invite Char6"
echo    REM timeout /t 2 /nobreak ^>nul
echo    REM cmd_all.bat "/invite"
echo.

:done
echo  Done.
pause
