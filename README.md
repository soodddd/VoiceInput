# VoiceInput

**v0.1.0-preview · Windows Local Voice Input**

[English](./README.md) | [简体中文](./README.zh-CN.md)

> ⚠️ **Preview Release** — This is an early preview version for testing and feedback. Expect bugs and breaking changes before the stable release.

A privacy-first, fully-local voice input tool for Windows. Speech recognition runs entirely on your GPU via [Qwen3-ASR](https://github.com/QwenLM/Qwen3) — no audio ever leaves your machine.

## Features

- 🎙️ **Push-to-talk** — Hold `Alt+V`, speak, release to transcribe and paste automatically
- 🌐 **Language switching** — `Alt+L` cycles Auto / Chinese / English
- 🔒 **100% offline** — All recognition runs on the local GPU, zero cloud calls
- 📝 **Custom terms** — Define ASR misrecognition → correct-text mappings
- ⚙️ **Settings panel** — Microphone selection, hotkey customization, audio tuning
- 🩺 **Auto-recovery** — Backend crash auto-restart + file logging
- 🖥️ **System tray** — Quick access to settings, logs, and quit

## System Requirements

| Requirement | Detail |
|-------------|--------|
| OS | Windows 10 1903+ / Windows 11 |
| GPU | NVIDIA CUDA 11.8+ compatible, ≥ 4 GB VRAM |
| Disk | ~200 MB app + ~1.2 GB model |
| RAM | 8 GB+ |
| Input | Microphone |

## Quick Start

1. **Download** the latest `VoiceInput-v0.1.0-preview-win64-preview.zip` from [Releases](../../releases)
2. **Extract** the zip to any folder (e.g. `C:\Program Files\VoiceInput\`)
3. **Run** `voiceinput.exe`
4. **First launch** — the app prompts you to download the ASR model (~1.2 GB, one-time)
5. **Press `Alt+V`**, speak, release — your words are pasted at the cursor

## Building from Source

### Prerequisites

- [Node.js](https://nodejs.org/) 18+ and npm
- [Rust](https://rustup.rs/) (stable toolchain)
- Python 3.10+ with the backend dependencies (`torch`, `fastapi`, `qwen_asr`, ...)
- [Tauri CLI 2.x](https://tauri.app/) (`npm install -D @tauri-apps/cli`)

### Steps

```bash
# 1. Install frontend dependencies
npm install

# 2. Build the Python ASR backend (PyInstaller onefile → src-tauri/binaries/)
.\build_backend.bat

# 3. Run in development mode
npm run tauri dev

# 4. Build the release zip (voiceinput.exe + sidecar + resources)
powershell -ExecutionPolicy Bypass -File .\build_release_zip.ps1
```

The release zip is output to `.\release\VoiceInput-v0.1.0-preview-win64-preview.zip`.

> **Note on NSIS** — The PyInstaller onefile sidecar is ~2.7 GB (bundling torch + transformers), which exceeds NSIS's mmap limit. The project ships a zip distribution instead. See `build_release_zip.ps1`.

## Architecture

```
voiceinput.exe (Tauri/Rust)  ──spawn──▶  asr_backend.exe (Python/FastAPI)
        │                                        │
        ├─ cpal (audio capture)                  ├─ Qwen3-ASR-0.6B (GPU inference)
        ├─ enigo (paste simulation)              ├─ uvicorn (HTTP server on 127.0.0.1:8765)
        ├─ rdev (global hotkeys)                 └─ postprocess (term correction)
        └─ React 18 (floating UI)
```

| Layer | Tech | Key Files |
|-------|------|-----------|
| Frontend | React 18 + TypeScript + Tailwind | `src/` |
| Native shell | Rust + Tauri 2 | `src-tauri/src/` |
| ASR backend | Python + FastAPI + PyTorch | `backend/` |

## Configuration

User config lives at `%LOCALAPPDATA%\VoiceInput\config.json`. Logs at `%LOCALAPPDATA%\VoiceInput\logs\`.

## Project Structure

```
voice-input-v2/
├─ src/                    # React frontend
├─ src-tauri/              # Rust + Tauri native shell
│  ├─ src/                 #   Rust source
│  ├─ binaries/            #   PyInstaller sidecar output (gitignored)
│  └─ tauri.conf.json      #   Tauri bundle config
├─ backend/                # Python ASR backend
│  ├─ server.py            #   FastAPI server
│  ├─ model_manager.py     #   Model download/load
│  ├─ postprocess.py       #   Text post-processing
│  └─ asr_backend.spec     #   PyInstaller spec
├─ resources/              # Default config, icon
├─ build_backend.bat       # Build sidecar EXE
├─ build_release_zip.ps1   # Build release zip
└─ docs/PRD.md             # Product requirements
```

## License

MIT — see [LICENSE](./LICENSE).

## Acknowledgements

- [Qwen3-ASR](https://github.com/QwenLM/Qwen3) — the ASR model
- [Tauri](https://tauri.app/) — the app framework
- [PyInstaller](https://pyinstaller.org/) — Python bundling
