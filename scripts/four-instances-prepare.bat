@echo off
rem Before a 4-instance nro run: closes Steam, Discord and WhatsApp, sets the game's graphics to windowed 960x540, all LOW (your settings are saved).
rem Double-click to run. The work is done by four-instances.ps1 next to this file.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0four-instances.ps1" -Prepare
pause
