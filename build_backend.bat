@echo off
REM ============================================================
REM  VoiceInput v2 — Build Python ASR Backend (PyInstaller)
REM  Outputs asr_backend.exe to src-tauri\binaries\
REM ============================================================

setlocal

set SCRIPT_DIR=%~dp0
set BACKEND_DIR=%SCRIPT_DIR%backend
set OUTPUT_DIR=%SCRIPT_DIR%src-tauri\binaries

echo [1/4] Checking Python environment...
python --version
if errorlevel 1 (
    echo ERROR: Python not found in PATH
    exit /b 1
)

echo [2/4] Installing PyInstaller...
pip install pyinstaller --quiet
if errorlevel 1 (
    echo ERROR: Failed to install PyInstaller
    exit /b 1
)

echo [3/4] Building asr_backend.exe...
cd /d "%BACKEND_DIR%"
pyinstaller asr_backend.spec --noconfirm --distpath "%OUTPUT_DIR%" --workpath "%TEMP%\voiceinput_pybuild"
if errorlevel 1 (
    echo ERROR: PyInstaller build failed
    exit /b 1
)

echo [4/4] Verifying output...
if exist "%OUTPUT_DIR%\asr_backend\asr_backend.exe" (
    echo SUCCESS: asr_backend.exe built at:
    echo   %OUTPUT_DIR%\asr_backend\asr_backend.exe
) else (
    echo WARNING: Expected output not found at expected path
    echo Check %OUTPUT_DIR% for the build output
)

endlocal
