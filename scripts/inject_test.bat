@echo off
REM ============================================================
REM  DMFT DLL Injection Test Script
REM  Finds eqgame.exe processes and injects dmft_dll.dll
REM ============================================================
setlocal enabledelayedexpansion

echo.
echo  === DMFT DLL Injection Test ===
echo.

REM --- Locate the DLL ---
set "DLL_PATH=%~dp0..\target\release\dmft_dll.dll"
if not exist "%DLL_PATH%" (
    set "DLL_PATH=%~dp0..\target\debug\dmft_dll.dll"
)
if not exist "%DLL_PATH%" (
    echo [ERROR] Cannot find dmft_dll.dll in target\release or target\debug.
    echo         Run: cargo build --release
    goto :eof
)
echo [OK] Found DLL: %DLL_PATH%
echo.

REM --- Locate the orchestrator ---
set "DMFT_EXE=%~dp0..\target\release\dmft.exe"
if not exist "%DMFT_EXE%" (
    set "DMFT_EXE=%~dp0..\target\debug\dmft.exe"
)
if not exist "%DMFT_EXE%" (
    echo [ERROR] Cannot find dmft.exe in target\release or target\debug.
    echo         Run: cargo build --release
    goto :eof
)
echo [OK] Found orchestrator: %DMFT_EXE%
echo.

REM --- Find eqgame.exe processes ---
echo Looking for eqgame.exe processes...
echo.

set COUNT=0
for /f "tokens=2" %%p in ('tasklist /fi "imagename eq eqgame.exe" /fo list ^| findstr "PID:"') do (
    set /a COUNT+=1
    set "PID_!COUNT!=%%p"
    echo   [!COUNT!] eqgame.exe  PID=%%p
)

if %COUNT%==0 (
    echo [ERROR] No eqgame.exe processes found. Launch EQ first.
    goto :eof
)
echo.
echo Found %COUNT% EQ client(s).
echo.

REM --- Inject into each ---
echo Starting injection...
echo.

"%DMFT_EXE%" --inject
if %ERRORLEVEL% neq 0 (
    echo.
    echo [WARN] Injection returned error code %ERRORLEVEL%.
) else (
    echo.
    echo [OK] Injection command completed.
)

echo.
echo  === Post-Injection Checks ===
echo.
echo  Log locations:
echo    Orchestrator: %~dp0..\logs\dmft.log
echo    DLL (injected): %TEMP%\dmft\dmft-dll.log
echo.
echo  To verify injection worked, run:
echo    scripts\verify_injection.bat
echo.
echo  Or check the DLL log:
echo    type "%TEMP%\dmft\dmft-dll.log"
echo.
