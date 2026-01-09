@echo off
title Alchemist Transcoding Server
echo ========================================
echo           ALCHEMIST SERVER
echo ========================================
echo.
echo Starting Alchemist in server mode...
echo Access the web UI at: http://localhost:3000
echo.
echo Press Ctrl+C to stop the server.
echo ========================================
echo.

cd /d "%~dp0"
alchemist.exe --server

echo.
echo Server stopped.
pause
