@echo off
echo ============================================
echo  Frostreaver - EQ Multibox Launcher
echo ============================================
echo.

cd /d "C:\Users\Public\Daybreak Game Company\Installed Games\EverQuest"

echo Launching EQ clients...
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

start "" eqgame.exe patchme /login:frostreaver06
echo  [5/6] frostreaver06 launched
timeout /t 5 /nobreak >nul

start "" eqgame.exe patchme /login:frostreaver07
echo  [6/6] frostreaver07 launched

echo.
echo  All clients launched.
echo  Enter passwords and select characters on each client.
echo.
echo  Press any key when all characters are in-game...
pause >nul

echo.
echo  Injecting DLL into all clients...
cd /d C:\Users\xmale\Projects\DMFT
target\release\dmft.exe --inject

echo.
echo  Waiting 3 seconds for hooks to initialize...
timeout /t 3 /nobreak >nul

echo.
echo  Starting TUI dashboard...
echo  (Press q to quit TUI)
echo.
target\release\dmft.exe
