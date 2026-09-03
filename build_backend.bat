@echo off
REM ============================================================
REM  VoiceInput v2 ? Build Python ASR Backend (PyInstaller)
REM  Outputs onedir runtime to src-tauri\binaries\asr_backend\
REM ============================================================

setlocal

set SCRIPT_DIR=%~dp0
set BACKEND_DIR=%SCRIPT_DIR%backend
set OUTPUT_DIR=%SCRIPT_DIR%src-tauri\binaries
set PYTHON=%SCRIPT_DIR%.venv-build\Scripts\python.exe
if not exist "%PYTHON%" set PYTHON=python

echo [1/4] Checking Python environment...
"%PYTHON%" --version
if errorlevel 1 (
    echo ERROR: Python not found in PATH
    exit /b 1
)

echo [2/4] Checking PyInstaller...
"%PYTHON%" -m PyInstaller --version
if errorlevel 1 (
    echo ERROR: PyInstaller is not installed in the active environment.
    echo Install the pinned backend requirements first; this script does not mutate environments.
    exit /b 1
)

echo [3/4] Building asr_backend.exe...
cd /d "%BACKEND_DIR%"
"%PYTHON%" -m PyInstaller asr_backend.spec --noconfirm --distpath "%OUTPUT_DIR%" --workpath "%TEMP%\voiceinput_pybuild"
if errorlevel 1 (
    echo ERROR: PyInstaller build failed
    exit /b 1
)

echo [4/4] Verifying output...
if exist "%OUTPUT_DIR%\asr_backend\asr_backend.exe" (
    if not exist "%OUTPUT_DIR%\asr_backend\_internal" (
        echo ERROR: _internal runtime directory is missing
        exit /b 1
    )
    "%PYTHON%" -m pip freeze --all > "%OUTPUT_DIR%\asr_backend\BUILD_REQUIREMENTS.txt"
    echo SUCCESS: asr_backend.exe built at:
    echo   %OUTPUT_DIR%\asr_backend\asr_backend.exe
) else (
    echo ERROR: Expected onedir output was not produced
    exit /b 1
)

endlocal
