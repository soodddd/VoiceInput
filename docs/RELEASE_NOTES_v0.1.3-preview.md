# VoiceInput v0.1.3-preview

Updated: 2026-09-05. Application/source version: **0.1.3**; release maturity: **preview**.

中文版摘要：Windows 本地语音输入工具，使用 NVIDIA GPU 运行 Qwen3-ASR。本次统一应用版本为 0.1.3，并更新中英文介绍、文档导航与发布边界。当前没有运行包附件；源码下载不能直接运行，完整语音识别与目标窗口输入仍需验收。

VoiceInput is a privacy-first Windows voice input tool. It records from a microphone, runs Qwen3-ASR locally on an NVIDIA GPU, and sends recognized text to the previous input window.

## What changed

- Unified React/Tauri/Rust/Python lifecycle and status handling
- Safer Windows text entry through Unicode `SendInput` without overwriting the clipboard
- More reliable WASAPI recording, device selection, VAD, silence trimming, normalization and bounded chunks
- Thread-safe model loading, unloading, downloading, cancellation and inference
- Random loopback port and per-run token; no cloud transcription and no startup GitHub check
- Backend crash monitoring, bounded upload size, clear errors and single-instance protection
- PyInstaller onedir backend with fresh-extraction release validation and SHA256 manifest
- Updated bilingual documentation, known boundaries and build instructions

## Validation

The following are historical build/smoke-test results, not a new end-to-end acceptance of every 0.1.3 component. Subsequent local testing identified premature VAD stopping and an older bundled backend; fixes and rebuilt package verification remain separate work.

- React/Vite build: passed
- Rust tests: 16 passed; Clippy with warnings denied: passed
- Python tests: 7 passed
- CUDA/Qwen3-ASR load and unload: passed on RTX 4060 Laptop GPU
- Microphone enumeration, recording, silence handling and two-second test: passed locally
- Final Tauri EXE startup and fresh release extraction/backend health check: passed

## Requirements

- Windows 10 1903+ or Windows 11
- NVIDIA CUDA GPU with at least 4 GB VRAM
- About 2.7 GB runtime space plus about 1.9 GB model space
- A microphone

## Distribution note

The complete onedir ZIP is about 2.73 GB and is not attached to this GitHub release. Build it with `build_release_zip.ps1` or use an external large-file release store. Always extract the package as a whole and keep `asr_backend/_internal` beside `asr_backend.exe`.

Historical local ZIP SHA256: `5743354396898B2FB02F5D55FD26532C245F8933944617562C06D8DD20F1D6B2`. This is **not an accepted 0.1.3 distribution**: local testing found the bundled backend still reported 0.1.2. Rebuild the backend and desktop together and verify their runtime versions before distributing a replacement.

This remains a preview release. Speech quality and automatic input behavior can vary by microphone, GPU driver, permissions and target application.
