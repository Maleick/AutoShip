@echo off
REM ============================================================
REM  Launch 6 EQ clients with stagger delay
REM  Copy to: C:\Users\xmale\Desktop\launch_eq.bat
REM ============================================================
setlocal

cd /d "C:\Users\Public\Daybreak Game Company\Installed Games\EverQuest"

echo.
echo  === Launching 6 EQ Clients ===
echo.

start "" eqgame.exe patchme /login:frostreaver01
echo  [1/6] frostreaver01 launched
timeout /t 5 /nobreak >nul

start "" eqgame.exe patchme /login:frostreaver02
echo  [2/6] frostreaver02 launched
timeout /t 5 /nobreak >nul

start "" eqgame.exe patchme /login:frostreaver03
echo  [3/6] frostreaver03 launched
timeout /t 5 /nobreak >nul

start "" eqgame.exe patchme /login:frostreaver04
echo  [4/6] frostreaver04 launched
timeout /t 5 /nobreak >nul

start "" eqgame.exe patchme /login:frostreaver05
echo  [5/6] frostreaver05 launched
timeout /t 5 /nobreak >nul

start "" eqgame.exe patchme /login:frostreaver06
echo  [6/6] frostreaver06 launched

echo.
echo  All 6 clients launched.
echo  Log in characters, then run: inject_and_group.bat
echo.
pause
