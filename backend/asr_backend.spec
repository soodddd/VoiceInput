# -*- mode: python ; coding: utf-8 -*-
"""PyInstaller spec for VoiceInput ASR Backend.

Builds an onedir ASR runtime including torch, FastAPI, uvicorn and
qwen_asr.  The directory starts directly instead of unpacking a
multi-gigabyte onefile archive on every application launch.

Output is placed in ../src-tauri/binaries/asr_backend and copied intact
by the custom ZIP release builder. Tauri externalBin bundling is disabled.
"""

import os
import sys
from PyInstaller.building.api import PYZ, EXE, COLLECT
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
    [],
    name='asr_backend',
    exclude_binaries=True,
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

coll = COLLECT(
    exe,
    a.binaries,
    a.zipfiles,
    a.datas,
    strip=False,
    upx=True,
    upx_exclude=[
        'torch',
        'torchvision',
        'torchaudio',
        'nvidia',
        'transformers',
        'qwen_asr',
    ],
    name='asr_backend',
)
