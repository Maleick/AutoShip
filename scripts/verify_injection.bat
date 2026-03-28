@echo off
REM ============================================================
REM  DMFT Injection Verification Script
REM  Checks: DLL loaded? Logs exist? Named pipe created?
REM ============================================================
setlocal enabledelayedexpansion

echo.
echo  === DMFT Injection Verification ===
echo.

set PASS=0
set FAIL=0

REM --- Check 1: Is eqgame.exe running? ---
echo [1/4] Checking for eqgame.exe processes...
set EQ_COUNT=0
for /f "tokens=2" %%p in ('tasklist /fi "imagename eq eqgame.exe" /fo list ^| findstr "PID:"') do (
    set /a EQ_COUNT+=1
    set "PID_!EQ_COUNT!=%%p"
)
if %EQ_COUNT%==0 (
    echo   FAIL: No eqgame.exe processes found.
    set /a FAIL+=1
    goto :check2
)
echo   OK: Found %EQ_COUNT% eqgame.exe process(es).
set /a PASS+=1

REM --- Check 2: Is dmft_dll.dll loaded in eqgame.exe? ---
:check2
echo.
echo [2/4] Checking if dmft_dll.dll is loaded in eqgame.exe...
set DLL_FOUND=0
for /L %%i in (1,1,%EQ_COUNT%) do (
    set "CPID=!PID_%%i!"
    REM Use tasklist /m to list modules loaded by the process
    tasklist /fi "pid eq !CPID!" /m 2>nul | findstr /i "dmft" >nul 2>&1
    if !ERRORLEVEL!==0 (
        echo   OK: PID !CPID! has dmft DLL loaded.
        set /a DLL_FOUND+=1
    ) else (
        REM Also check for randomized DLL names from dll_prep staging
        tasklist /fi "pid eq !CPID!" /m 2>nul | findstr /i "msvc_ dx_ d3d_ win_ sys_ rt_ api_ cfg_ net_ sec_" >nul 2>&1
        if !ERRORLEVEL!==0 (
            echo   OK: PID !CPID! has staged DLL loaded (randomized name).
            set /a DLL_FOUND+=1
        ) else (
            echo   FAIL: PID !CPID! does NOT have dmft DLL loaded.
        )
    )
)
if %DLL_FOUND% gtr 0 (
    set /a PASS+=1
) else (
    set /a FAIL+=1
)

REM --- Check 3: DLL log file exists and has recent entries? ---
echo.
echo [3/4] Checking DLL log file...
set "LOG_FILE=%TEMP%\dmft\dmft-dll.log"

REM tracing-appender rolling::daily appends the date to the filename
set "TODAY=%date:~-4%-%date:~4,2%-%date:~7,2%"

REM Check for any dmft-dll log files in the temp dir
set LOG_FOUND=0
for %%f in ("%TEMP%\dmft\dmft-dll.log*") do (
    set LOG_FOUND=1
    set "FOUND_LOG=%%f"
)

if %LOG_FOUND%==1 (
    echo   OK: DLL log found: %FOUND_LOG%
    echo   Last 5 lines:
    echo   ---
    for /f "tokens=*" %%l in ('powershell -command "Get-Content '%FOUND_LOG%' -Tail 5"') do (
        echo     %%l
    )
    echo   ---
    set /a PASS+=1
) else (
    echo   FAIL: No DLL log file found at %TEMP%\dmft\dmft-dll.log*
    echo         The DLL either failed to load or failed to initialize tracing.
    set /a FAIL+=1
)

REM --- Check 4: Named pipe created? ---
echo.
echo [4/4] Checking for DMFT named pipes...
set PIPE_FOUND=0
for /L %%i in (1,1,%EQ_COUNT%) do (
    set "CPID=!PID_%%i!"
    REM Check if the pipe exists using PowerShell
    powershell -command "if (Test-Path '\\.\pipe\dmft_cmd_!CPID!') { exit 0 } else { exit 1 }" 2>nul
    if !ERRORLEVEL!==0 (
        echo   OK: Pipe \\.\pipe\dmft_cmd_!CPID! exists.
        set /a PIPE_FOUND+=1
    ) else (
        echo   WARN: Pipe \\.\pipe\dmft_cmd_!CPID! not found (IPC may not have started).
    )
)
if %PIPE_FOUND% gtr 0 (
    set /a PASS+=1
) else (
    set /a FAIL+=1
)

REM --- Summary ---
echo.
echo  ===================================
echo   Results: %PASS% passed, %FAIL% failed
echo  ===================================
echo.
if %FAIL%==0 (
    echo  ALL CHECKS PASSED - Injection looks good!
) else (
    echo  Some checks failed. Review output above.
    echo  Check logs:
    echo    Orchestrator: logs\dmft.log
    echo    DLL: %TEMP%\dmft\dmft-dll.log*
)
echo.
