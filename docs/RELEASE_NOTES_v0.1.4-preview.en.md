# VoiceInput v0.1.4-preview

Release date: 2026-09-05
Application version: **0.1.4**
Release channel: **Preview**

## Summary

0.1.4 moves VoiceInput from “the feature set is implemented” toward “it can be used repeatedly and released through one reproducible path.” It keeps fully local Qwen3-ASR recognition and focuses on Windows WASAPI recorder lifetime, responsive level feedback, and version/documentation consistency.

## What changed

### 1. More reliable recording

- The `cpal`/WASAPI input stream is created, owned, and released on a dedicated worker thread, reducing lifecycle conflicts between Tauri command threads and Windows audio sessions.
- `start_recording` waits for the stream to be ready before returning. Startup failures and timeouts wait for the worker to exit instead of leaving an orphan recorder thread.
- Default-device capture, device-index selection, and device-name selection remain supported. The recorder uses the device's complete default format; the Python backend performs the final 16 kHz resampling.
- The recording buffer keeps a maximum-duration bound so a long session cannot grow memory without limit.

### 2. Smoother level feedback and VAD

- The realtime audio callback only converts samples, maintains the bounded buffer, computes RMS, and stores an atomic snapshot. It no longer synchronously crosses into the WebView.
- A lightweight sampler sends `audio-level` every 50 ms, keeping the floating waveform and Settings → Microphone → Test responsive.
- VAD timing starts after the audio stream is actually running, so WASAPI initialization time is not misclassified as user silence.
- Minimum recording time, continuous-silence detection, and maximum-duration protection remain enabled.
- Stop logs include total RMS and peak; VAD logs include current and maximum RMS, making “no captured audio” distinguishable from “the model did not recognize it.”

### 3. Hardware diagnostic probe

The new `audio_hardware_probe` developer tool can:

- list the default input/output devices and actual input configuration;
- play a WAV through the default speaker/Bluetooth speaker while capturing the default microphone;
- report sample count, RMS, peak, and the fraction of samples above thresholds;
- use `--no-playback` to establish a silence baseline and isolate speaker, microphone, routing, or VoiceInput issues.

It is a development diagnostic, not the normal application entry point and not a replacement for the transcription flow.

### 4. Version and distribution consistency

The following are synchronized at `0.1.4`:

- `package.json` / `package-lock.json`
- `src-tauri/Cargo.toml` / `src-tauri/Cargo.lock`
- `src-tauri/tauri.conf.json`
- the startup log in `backend/__main__.py`
- FastAPI metadata in `backend/server.py`
- `index.html`, both READMEs, and the documentation index

The release script continues to use a PyInstaller onedir backend and validates complete extraction, the relative `asr_backend/_internal` layout, a SHA-256 manifest, and `/health` from a freshly extracted sidecar.

The same-batch package passed validation: `VoiceInput-v0.1.4-preview-win64.zip`, about 2,727.5 MB, SHA-256 `592E2B6A8F196AC04E0DF1AF162FBEB1FD1A62F04CF723719047B2B3D9B69E4E`. Because the complete ZIP exceeds the GitHub Release per-asset limit, the GitHub Release does not upload this large file; source, bilingual documentation, and the reproducible build path are published normally.

## Upgrading from 0.1.3

1. Exit the old VoiceInput instance.
2. Replace `voiceinput.exe` and the complete `asr_backend` directory together; do not replace only one component.
3. Keep `%LOCALAPPDATA%\VoiceInput\models`; existing model caches remain usable.
4. After launch, open Settings → Microphone → Test.
5. Focus a normal Notepad or browser text field, hold `Alt+V`, speak a short sentence, release it, and confirm both recognition and automatic entry.

The configuration remains at `%LOCALAPPDATA%\VoiceInput\config.json`, and logs remain at `%LOCALAPPDATA%\VoiceInput\logs\`.

## Validation status

### Automated validation

- TypeScript/Vite build: passed
- Rust unit tests: passed
- Python backend tests: passed
- Release build: desktop and sidecar must be rebuilt from the same source state
- Fresh-extraction validation: requires the onedir `_internal` directory and a passing `/health`

### Local and field validation

- The default output is a Bluetooth headset/speaker and the default input is the Intel Smart Sound microphone array. The independent probe captures playback at about `-20 dB RMS`; the silence baseline is about `-104 dB RMS`.
- The user's field GUI evidence shows “recognition completed,” the recognized text rendered in the result area, and the text entered into the target window.
- Elevated windows, games, custom controls, and remote-desktop sessions still need target-specific acceptance.

## Known boundaries

- Windows 10 1903+ / Windows 11 and an NVIDIA CUDA GPU are required; CPU, AMD, and Intel GPU inference paths are not provided.
- The runtime package is about 2.7 GB and the model cache about 1.9 GB; plan storage for both.
- GitHub's generated source archive is not a runnable onedir package; extract the complete runtime package.
- Automatic entry depends on the target application accepting Windows input events. An elevated target may also require VoiceInput to run elevated.
- Preview does not mean every hardware route and every window type has been certified.

## Rollback

If 0.1.4 regresses in a target application, exit VoiceInput and restore the previous version's complete `voiceinput.exe` and `asr_backend` directory. Do not mix desktop and sidecar versions. The model cache normally does not need to be rolled back.
