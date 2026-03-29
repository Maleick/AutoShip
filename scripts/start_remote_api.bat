@echo off
REM Start DMFT Remote API Server
REM Run as Administrator for port binding, or run the netsh command first:
REM   netsh http add urlacl url=http://+:8080/ user=%USERNAME%
powershell -ExecutionPolicy Bypass -File "%~dp0remote_api.ps1" -Port 8080
