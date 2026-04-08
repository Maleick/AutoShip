@echo off
REM Start TextQuest Remote API Server
REM Binds to localhost only by default (http://localhost:8080/)
powershell -ExecutionPolicy Bypass -File "%~dp0remote_api.ps1" -Port 8080
