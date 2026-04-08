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

REM --- Locate textquest.exe ---
set "TextQuest_EXE=%~dp0..\target\release\textquest.exe"
if not exist "%TextQuest_EXE%" (
    set "TextQuest_EXE=%~dp0..\target\debug\textquest.exe"
)
if not exist "%TextQuest_EXE%" (
    echo [ERROR] Cannot find textquest.exe. Run: cargo build --release
    goto :eof
)

REM --- Find all eqgame.exe PIDs and send command ---
set COUNT=0
for /f "tokens=2" %%p in ('tasklist /fi "imagename eq eqgame.exe" /fo list ^| findstr "PID:"') do (
    set /a COUNT+=1
    echo  [!COUNT!] PID=%%p  sending: %CMD%
    "%TextQuest_EXE%" --cmd %%p "%CMD%"
)

if %COUNT%==0 (
    echo [ERROR] No eqgame.exe processes found.
) else (
    echo.
    echo  Sent "%CMD%" to %COUNT% client(s).
)
