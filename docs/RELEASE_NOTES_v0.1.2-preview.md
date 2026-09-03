# VoiceInput v0.1.2-preview

VoiceInput is a privacy-first Windows voice input tool. It records from a microphone, runs Qwen3-ASR locally on an NVIDIA GPU, and sends the recognized text to the previous input window.

## Highlights

- Push-to-talk with `Alt+V`, click-to-record, and `Alt+L` language switching
- Local Qwen3-ASR inference through a Python/FastAPI sidecar
- Windows `SendInput` text entry without overwriting the clipboard
- Microphone selection and two-second microphone test
- VAD silence auto-stop, custom terms, punctuation and model-memory strategies
- Random loopback port, per-run token, bounded uploads and no cloud transcription
- Crash monitoring, single-instance protection, structured logs and onedir packaging

## Validation

- React/Vite production build: passed
- Rust tests: 16 passed; Clippy with warnings denied: passed
- Python tests: 7 passed
- CUDA/Qwen3-ASR load, inference setup and unload: passed on RTX 4060 Laptop GPU
- Microphone enumeration, recording, silence handling and microphone test: passed locally
- Fresh release extraction, SHA256 manifest, `_internal` layout and backend health check: passed

## Requirements

- Windows 10 1903+ or Windows 11
- NVIDIA GPU with CUDA support and at least 4 GB VRAM
- About 2.7 GB for the runtime and 1.9 GB for the model
- A microphone

## Distribution note

The complete onedir ZIP is about 2.73 GB and is not attached to this GitHub release. Downloading and committing that artifact to the repository is intentionally avoided; build it with `build_release_zip.ps1` or use an external large-file release store. The package must be extracted as a whole, with `asr_backend/_internal` beside `asr_backend.exe`.

This is a preview release. Real speech recognition quality and input behavior can vary by microphone, GPU driver, permissions, and target application.
