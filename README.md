# VoiceInput

**v0.1.3-preview · Windows Local Voice Input**

Application version: **0.1.3**. The GitHub release keeps the `-preview` label until end-to-end acceptance is complete. See the [documentation index](./docs/README.md).

[English](./README.md) | [简体中文](./README.zh-CN.md)

[Latest Release](../../releases/tag/v0.1.3-preview) | [Fix and validation report](./docs/CODE_REVIEW_FIX_REPORT_2026-09-03.md) | [Release notes](./docs/RELEASE_NOTES_v0.1.3-preview.md)

> ⚠️ **Preview Release** — This is an early preview version for testing and feedback. Expect bugs and breaking changes before the stable release.

A privacy-first, fully-local voice input tool for Windows. Speak into your microphone and your words appear wherever your cursor is — no typing needed. Speech recognition runs entirely on your GPU via [Qwen3-ASR](https://github.com/QwenLM/Qwen3), so **no audio ever leaves your computer**.

This is a Windows desktop preview built with React, Tauri, Rust, and Python. The desktop layer handles recording, hotkeys, and safe text entry; the Python service listens only on a random local loopback port and runs the model on a local NVIDIA GPU.

### Current status

- ✅ Source, tests, desktop app, and the onedir backend build successfully
- ✅ Local NVIDIA GPU, Qwen3-ASR model loading, and microphone recording have been verified
- ⚠️ Preview release: real speech recognition and target-application input still need confirmation on the target machine

---

## What is VoiceInput? (For Everyone)

VoiceInput is like having a friend who types everything you say. Instead of using your keyboard, you just:

1. **Hold a key** (Alt+V) on your keyboard
2. **Talk** into your microphone
3. **Let go** of the key
4. Your words **magically appear** where your cursor is blinking!

It works in most standard text fields — Word, chat boxes, and browser search bars. Some elevated windows, games, and custom-drawn controls may block simulated input and need separate validation.

---

## Features

- 🎙️ **Push-to-talk** — Hold `Alt+V`, speak, release to transcribe and paste automatically
- 🖱️ **Click to talk** — Click the microphone button, click again to stop
- 🌐 **Language switching** — `Alt+L` cycles Auto / Chinese / English
- 🔒 **100% offline** — All recognition runs on the local GPU, zero cloud calls
- 📝 **Custom terms** — Define ASR misrecognition → correct-text mappings
- 🩺 **Auto-recovery** — Backend crash auto-restart + file logging
- 🖥️ **System tray** — Quick access to settings, logs, and quit
- ⚡ **VAD auto-stop** — Detects silence and stops recording automatically
- 📋 **Clipboard-safe input** — Returns to the prior window and types without altering clipboard data

---

## System Requirements

| Requirement | Detail |
|-------------|--------|
| OS | Windows 10 1903+ / Windows 11 |
| GPU | **NVIDIA** graphics card with CUDA support, ≥ 4 GB VRAM |
| Disk | ~2.7 GB runtime + ~1.9 GB model (downloaded once); keep 6 GB free |
| RAM | 8 GB+ |
| Input | A microphone (built-in or external) |

> ⚠️ **Important:** An **NVIDIA** graphics card is required. Intel/AMD integrated graphics are not supported. If you're not sure what GPU you have, look for "NVIDIA" in your Task Manager → Performance tab.

---

## Quick Start (Step by Step)

### Step 1: Download

1. Go to the [Releases page](../../releases)
2. This release currently has **no downloadable runtime asset**. GitHub's source archives are not runnable packages. Build from source using the development instructions below; rebuild both the Python backend and desktop app before packaging.
3. When a verified `VoiceInput-v0.1.3-preview-win64.zip` becomes available, use the extraction steps below. The previous local ZIP is not a verified 0.1.3 distribution.

### Step 2: Extract

1. Find the downloaded `.zip` file (usually in your Downloads folder)
2. **Right-click** the zip file → **Extract All...**
3. Choose a location, for example: `C:\VoiceInput`
4. Click **Extract**

> 💡 **Tip:** Don't put it in a folder that needs admin permission (like `C:\Program Files\`), or it might not work properly.

### Step 3: Run

1. Open the extracted folder
2. **Double-click** `voiceinput.exe`
3. A small floating window will appear on your screen

> 🛡️ If Windows shows a "Windows protected your PC" warning, click **More info** → **Run anyway**. This is normal for apps that aren't from the Microsoft Store.

### Step 4: Download the Model (First Launch Only)

The first time you run VoiceInput, it needs to download the AI model (about 1.9 GB). This happens only once.

1. The app will show a download screen
2. Choose **ModelScope** (recommended, faster for China) or **HuggingFace** (international)
3. Click **开始下载 (Start Download)**
4. Wait for the download to finish (this depends on your internet speed, usually 5-20 minutes)
5. Click **加载模型 (Load Model)** — this takes about 10-20 seconds

### Step 5: Start Talking!

1. Open any program where you can type (e.g., a Word document, a chat box, a search bar)
2. Click where you want the text to appear
3. **Press and hold `Alt+V`** on your keyboard
4. **Speak** into your microphone
5. **Release `Alt+V`**
6. Your words will appear automatically!

---

## How to Use

### The Floating Window

The small window on your screen has these parts:

- **Microphone button** (big circle): Click to start/stop recording
- **Language badge** (top-left, shows "Auto"/"中"/"EN"): Click to switch language
- **Gear icon** (top-right): Click to open Settings
- **X icon** (top-right): Click to hide the window (app keeps running in system tray)

### Keyboard Shortcuts

| Shortcut | What it does |
|----------|-------------|
| `Alt+V` (hold) | Push-to-talk: hold while speaking, release to transcribe |
| `Alt+L` | Switch language: Auto → Chinese → English → Auto |

### System Tray

A green microphone icon appears in your system tray (bottom-right of screen, near the clock). **Right-click** it to:
- Show/hide the panel
- Open Settings
- Switch language
- Load/unload the model (free GPU memory)
- View log files
- Quit the app

---

## Settings

Open Settings by clicking the **gear icon** on the floating window, or right-click the tray icon → **设置**.

### Microphone Tab
Choose which microphone to use. Click **Test** to record a short sample and see if it works.

### Hotkey Tab
Change the push-to-talk hotkey (default: Alt+V) and language-switch hotkey (default: Alt+L).

### Audio Tab
Adjust sample rate, audio normalization, silence trimming, and silence threshold.

### Advanced Tab
- **Model strategy**: How the AI model uses GPU memory
  - **Balanced** (default): Unloads after 30 minutes idle
  - **Performance**: Always loaded (fastest response)
  - **Memory saver**: Unloads after each use (lowest VRAM)
  - **Accurate**: Maximum precision
- **Punctuation mode**: How punctuation is handled
  - **Simple** (default): Auto-adds periods at the end
  - **Raw**: No changes
  - **Input method**: Removes end punctuation (good for search boxes)
- **Auto space (Chinese-English)**: Adds spaces between Chinese and English
- **VAD silence detection**: Automatically stops recording after 2 seconds of silence
- **Auto-start on boot**: Start VoiceInput when Windows starts
- Input delay, max recording time, request timeout (automatic input does not touch the clipboard)

### Terms Tab
Add custom word corrections. For example, if the AI always mishears your name "Xiaoming" as "Xiao Ming", add a rule to fix it.

---

## Known boundaries

- Windows 10 1903+ / Windows 11 and an NVIDIA CUDA GPU are required; CPU, AMD, and Intel GPU inference are not supported.
- The model download is about 1.9 GB; the release ZIP is about 2.7 GB, so plan for both separately.
- Automatic entry depends on the target application accepting Windows input events; elevated windows, custom controls, and some games may reject it.
- Compiled binaries and model caches are not committed to the repository. Download the complete release package and keep `asr_backend/_internal` beside the backend executable.

## Troubleshooting

### The app won't start / closes immediately
- **Most likely cause:** No NVIDIA GPU. VoiceInput requires an NVIDIA graphics card. Check Task Manager → Performance tab for "NVIDIA".
- **Solution:** Install the latest [NVIDIA driver](https://www.nvidia.com/Download/index.aspx). If you have a laptop with both Intel and NVIDIA, set VoiceInput to use NVIDIA in NVIDIA Control Panel.

### A dialog says "another instance is already running"
- VoiceInput is already open. Look for the green microphone icon in your system tray (bottom-right of screen).

### Recording starts but no text appears
- **Check:** Is the model loaded? Open Settings → Advanced → Model strategy. If it says "not loaded", click the tray icon → **预加载模型**.
- **Check:** Is your microphone working? Open Settings → Microphone → Test.

### Text appears in the wrong place
- VoiceInput pastes text at your **cursor position**. Click where you want the text before speaking.
- Some apps (like certain games) may block paste. Try clicking in a text box first.

### Recognition is slow
- First recognition after loading takes longer (the model is warming up). Subsequent ones are faster.
- Try changing Model Strategy to **Performance** (always loaded) in Settings.
- Make sure no other heavy GPU programs are running.

### Download fails / is very slow
- Choose **ModelScope** if you're in China (faster), or **HuggingFace** for international.
- The download supports resume — if it stops, just click download again to continue.

---

## Building from Source

### Prerequisites

- [Node.js](https://nodejs.org/) 20.19+ and npm
- [Rust](https://rustup.rs/) (stable toolchain)
- Python 3.10+ with backend dependencies (`torch`, `fastapi`, `qwen_asr`, ...)
- [Tauri CLI 2.x](https://tauri.app/) (`npm install -D @tauri-apps/cli`)
- NVIDIA CUDA Toolkit 11.8+

### Steps

```bash
# 1. Install frontend dependencies
npm install

# 2. Install backend dependencies and a fixed packager in a dedicated environment
python -m pip install -r backend\requirements.txt
python -m pip install pyinstaller==6.22.1

# 3. Build the Python ASR backend (PyInstaller onedir → src-tauri/binaries/asr_backend/)
.\build_backend.bat

# 4. Build the Tauri executable
npm run tauri -- build

# 5. Build and fresh-extraction validate the release zip
powershell -ExecutionPolicy Bypass -File .\build_release_zip.ps1
```

The release zip is output to `.\release\VoiceInput-v0.1.3-preview-win64.zip`.

> **Packaging note** — The backend is an onedir runtime (~2.7 GB including torch and transformers) to avoid unpacking a giant executable on every launch. The project ships a ZIP; keep the complete `asr_backend/_internal` directory beside `asr_backend.exe`.

---

## Architecture

```
voiceinput.exe (Tauri/Rust)  ──spawn──▶  asr_backend.exe (Python/FastAPI)
        │                                        │
        ├─ cpal (audio capture)                  ├─ Qwen3-ASR-0.6B (GPU inference)
        ├─ Windows SendInput (safe text entry)   ├─ uvicorn (random 127.0.0.1 port)
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

---

## Changelog

### v0.1.3-preview (Latest)

This release is a full stability, privacy, and distribution upgrade covering local GPU inference, recording, model lifecycle, safe text entry, recovery, and onedir packaging, with local validation completed.

**New Features:**
- 📋 **Single instance lock** — Prevents running two copies at once
- 🔒 **Local loopback transport** — Uses a random port and a fresh token per run

**Improvements:**
- 💬 **Friendly error messages** — Errors now show clear Chinese explanations (e.g., "microphone not found" instead of raw error codes)
- 🖥️ **GPU error dialog** — When NVIDIA GPU is not found, a clear popup explains the problem instead of silently crashing
- ⚙️ **Settings panel** — Increased height to fit all new settings without scrolling issues
- 📝 **Default config** — Added missing P2 feature fields (auto_start, punctuation, VAD, spacing)

**Bug Fixes:**
- Fixed tray "Settings" menu not working (was looking for a nonexistent window)
- Fixed `process_manager.rs` comment missing "memory" strategy
- Fixed `build_release_zip.ps1` hardcoded version string

### v0.1.1-preview

**New Features:**
- 🧠 **Memory strategy** — New model strategy that releases VRAM after each recognition
- 🔇 **VAD silence detection** — Auto-stops recording after 2 seconds of silence
- 🚀 **Auto-start on boot** — Optionally start VoiceInput when Windows starts
- 〽️ **Punctuation modes** — Choose how punctuation is handled (raw/simple/input_method)
- 🀄 **Chinese-English spacing** — Auto-adds spaces between Chinese and English text
- `/model/strategy` API endpoints for runtime strategy switching

### v0.1.0-preview

**Initial Release:**
- Core push-to-talk voice input (Alt+V)
- 100% local Qwen3-ASR recognition
- Auto-paste at cursor position
- Multi-language support (Auto/Chinese/English)
- Model download from ModelScope/HuggingFace
- System tray, settings panel, custom terms
- Backend crash auto-restart
