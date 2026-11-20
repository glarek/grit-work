@echo off
setlocal
set "grit_ocr_powershell=%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe"
if exist "%SystemRoot%\Sysnative\WindowsPowerShell\v1.0\powershell.exe" set "grit_ocr_powershell=%SystemRoot%\Sysnative\WindowsPowerShell\v1.0\powershell.exe"
"%grit_ocr_powershell%" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "%~dp0install-ocr-languages.ps1"
set "grit_ocr_exit=%errorlevel%"
if "%grit_ocr_exit%"=="0" echo Swedish and English OCR are ready.
if "%grit_ocr_exit%"=="3010" echo Restart Windows before using OCR.
if not "%grit_ocr_exit%"=="0" if not "%grit_ocr_exit%"=="3010" echo OCR setup failed. Check Windows Update and allow the administrator prompt, then run setup again.
pause
exit /b %grit_ocr_exit%
