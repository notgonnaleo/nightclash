@echo off
rem After a 4-instance nro run (stop the games first: nro stop --all): puts your own graphics settings back.
rem Double-click to run. The work is done by four-instances.ps1 next to this file.
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0four-instances.ps1" -Restore
pause
