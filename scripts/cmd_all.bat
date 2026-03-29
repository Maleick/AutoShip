@echo off
REM ============================================================
REM  Send a slash command to ALL injected EQ clients
REM  Usage: cmd_all.bat "/sit"
REM         cmd_all.bat "/say Hello everyone"
REM ============================================================
setlocal enabledelayedexpansion

if "%~1"=="" (
    echo  Usage: cmd_all.bat "/command"
    echo  Example: cmd_all.bat "/sit"
    goto :eof
)

set "CMD=%~1"

REM --- Locate dmft.exe ---
set "DMFT_EXE=%~dp0..\target\release\dmft.exe"
if not exist "%DMFT_EXE%" (
    set "DMFT_EXE=%~dp0..\target\debug\dmft.exe"
)
if not exist "%DMFT_EXE%" (
    echo [ERROR] Cannot find dmft.exe. Run: cargo build --release
    goto :eof
)

REM --- Find all eqgame.exe PIDs and send command ---
set COUNT=0
for /f "tokens=2" %%p in ('tasklist /fi "imagename eq eqgame.exe" /fo list ^| findstr "PID:"') do (
    set /a COUNT+=1
    echo  [!COUNT!] PID=%%p  sending: %CMD%
    "%DMFT_EXE%" --cmd %%p "%CMD%"
)

if %COUNT%==0 (
    echo [ERROR] No eqgame.exe processes found.
) else (
    echo.
    echo  Sent "%CMD%" to %COUNT% client(s).
)
