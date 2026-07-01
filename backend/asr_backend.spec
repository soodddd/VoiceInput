# -*- mode: python ; coding: utf-8 -*-
"""PyInstaller spec for VoiceInput ASR Backend.

Builds asr_backend.exe as a single-file executable (onefile mode) from
the Python backend, including all runtime dependencies (torch, fastapi,
uvicorn, qwen_asr, etc.).

Output is placed in ../src-tauri/binaries/ for Tauri sidecar bundling.
Tauri's ``externalBin`` requires a single .exe file, so onefile mode is
mandatory (onedir would produce a directory that Tauri cannot consume
via externalBin).
"""

import os
import sys
from PyInstaller.building.api import PYZ, EXE
from PyInstaller.building.build_main import Analysis
from PyInstaller.utils.hooks import collect_submodules, collect_data_files

block_cipher = None

# Backend source directory
backend_dir = os.path.dirname(os.path.abspath(SPEC))

# Whole-package collection: qwen_asr/nagisa/soynlp have internal cross-
# imports that PyInstaller's static analysis misses, causing
# ModuleNotFoundError at runtime. Force-collecting every submodule
# and data file from these packages is the standard fix.
qwen_asr_submods = collect_submodules('qwen_asr')
nagisa_submods = collect_submodules('nagisa')
soynlp_submods = collect_submodules('soynlp')

a = Analysis(
    [os.path.join(backend_dir, '__main__.py')],
    pathex=[backend_dir],
    binaries=[],
    datas=[
        *collect_data_files('qwen_asr'),
        *collect_data_files('nagisa'),
        *collect_data_files('soynlp'),
    ],
    hiddenimports=[
        'uvicorn.logging',
        'uvicorn.loops',
        'uvicorn.loops.auto',
        'uvicorn.protocols',
        'uvicorn.protocols.http',
        'uvicorn.protocols.http.auto',
        'uvicorn.protocols.websockets',
        'uvicorn.protocols.websockets.auto',
        'uvicorn.lifespan',
        'uvicorn.lifespan.on',
        'modelscope',
        'huggingface_hub',
        'soundfile',
        'scipy',
        'scipy.signal',
        'fastapi',
        'fastapi.middleware',
        'fastapi.middleware.cors',
        'pydantic',
        # Force-include all submodules of packages with tricky
        # cross-imports that static analysis misses.
        *qwen_asr_submods,
        *nagisa_submods,
        *soynlp_submods,
    ],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[],
    win_no_prefer_redirects=False,
    win_private_assemblies=False,
    cipher=block_cipher,
    noarchive=False,
)

pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.zipfiles,
    a.datas,
    [],
    name='asr_backend',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    upx_exclude=[
        # torch/torchvision C extensions may break under UPX
        'torch',
        'torchvision',
        'torchaudio',
        'nvidia',
        'transformers',
        'qwen_asr',
    ],
    console=True,
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
)
