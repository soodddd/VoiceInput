# VoiceInput 0.1.4-preview Validation Report (English)

Date: 2026-09-05
Repository: `E:\ASK\voice-input-v2`
Branch: `master`

## Conclusion

0.1.4 has completed source-level, build-level, hardware-comparison, and field usability checks. It is suitable for Preview distribution. The evidence shows that the microphone, Bluetooth output, and independent `cpal` capture path are healthy; the application recorder is usable after worker-thread lifecycle isolation; and the user's field screenshot shows recognition completion, result rendering, and target-window entry.

“Preview” still means that every privilege level, input-control type, and audio-driver combination has not been certified. It does not mean the current feature is unusable.

## Validation matrix

| Layer | Check | Result | Evidence/command |
|---|---|---|---|
| Frontend | TypeScript + Vite build | PASS | `npm run build` |
| Rust | Unit tests | PASS | `cargo test --manifest-path src-tauri/Cargo.toml` |
| Python | Backend tests | PASS | `.venv-build\\Scripts\\python.exe -m unittest discover -s backend/tests -v` |
| Diagnostic | `audio_hardware_probe` compilation | PASS | `cargo check --manifest-path src-tauri/Cargo.toml --bin audio_hardware_probe` |
| Sidecar | Version/startup log | PASS | `Starting VoiceInput ASR backend v0.1.4` |
| Audio hardware | Bluetooth playback → microphone capture | PASS | about `-20 dB RMS`, peak about `-0.1 dB` |
| Audio baseline | Capture with no playback | PASS | about `-104 dB RMS`, 0 samples above -40 dB |
| Desktop | Real recording, recognition, result display | PASS | User field GUI screenshot shows completed recognition and Chinese result |
| Target input | Automatic entry into target text window | PASS | User field GUI screenshot shows result in target window |
| Release package | Fresh extraction + `_internal` + `/health` | PASS | `release/VoiceInput-v0.1.4-preview-win64.zip` |

## Desktop end-to-end smoke test

On the same Windows validation machine, 0.1.4 was launched from `src-tauri\target\release\voiceinput.exe`. The window title was confirmed as `VoiceInput 0.1.4`; Qwen3-ASR-0.6B was loaded from the local cache, and one end-to-end test was executed:

1. Open Notepad and focus its editor.
2. Click the VoiceInput microphone button.
3. Play the repository fixture `test-artifacts\speech\chinese.wav` through the default speaker.
4. Wait for automatic recording stop and local GPU inference.
5. Check both the VoiceInput result bubble and the Notepad target window.

Log summary:

```text
Device: 麦克风阵列 (适用于数字麦克风的英特尔® 智音技术)
Format: 48000 Hz / 2 channels / F32
Recording: about 4.8 s, RMS=0.034215, Peak=0.855103
Recognition: 14 characters, about 2269 ms processing, about 3070 ms effective audio
Input: recognition result entered (14 characters)
Sidecar: POST /transcribe -> 200 OK, /health.version=0.1.4
```

The result was rendered in VoiceInput and entered into Notepad successfully. This run is independent of the user's supplied field screenshot, while reaching the same conclusion: the current build completes “recording -> local recognition -> target-window input”.

## Hardware comparison

Default devices on the validation machine:

- Output: Bluetooth headset/speaker `耳机 (COLORFIRE CL500)`
- Input: `麦克风阵列 (适用于数字麦克风的英特尔® 智音技术)`
- Input format: `48000 Hz / 2 channels / F32`

The independent probe played a fixed WAV and captured for 12 seconds:

```text
Playback: RMS=0.094655 (about -20.5 dB), Peak=0.993988 (about -0.1 dB)
Playback: 217364 samples above -40 dB (20.76%)
Silence baseline: RMS=0.000006 (about -104.2 dB), Peak=0.000336 (about -69.5 dB)
Silence baseline: 0 samples above -40 dB (0.00%)
```

The independent probe was also run concurrently while VoiceInput was recording. Playback remained about `-20.8 dB RMS`, showing that Bluetooth output and microphone input do not fail merely because VoiceInput is running. This narrowed the earlier investigation to the application's recorder lifecycle rather than hardware replacement.

## Release package credentials

The same-batch ZIP completed fresh-directory extraction and sidecar `/health` version validation:

```text
File: VoiceInput-v0.1.4-preview-win64.zip
Size: 2,727.5 MB (2,859,943,059 bytes)
SHA-256: 592E2B6A8F196AC04E0DF1AF162FBEB1FD1A62F04CF723719047B2B3D9B69E4E
Checks: fresh extraction / asr_backend/_internal / SHA256SUMS / /health.version=0.1.4
```

Because the complete ZIP exceeds the GitHub Release per-asset limit, this report keeps the local release credential; the GitHub Release publishes source and documentation rather than uploading an asset that could be truncated.

## 0.1.4 code checks

### Recorder thread

In `src-tauri/src/recorder.rs`, a session creates and owns `cpal::Stream` on a dedicated worker. Stop sets the atomic stop flag, waits for the worker, and only then reads the buffer. WASAPI streams are therefore released on their creation thread, and startup failures do not leave detached recorder threads.

### Level feedback

The realtime callback no longer synchronously invokes the WebView `audio-level` event. It stores an atomic RMS snapshot; a separate sampler emits the event every 50 ms. The frontend `useRecorder` hook and microphone test continue to use the existing event protocol without configuration migration.

### VAD

`record_start` and `last_sound_time` are reset after `stream.play()` succeeds, so device initialization time is not counted as silence. Stop logs include total RMS and peak, and VAD logs include the maximum RMS observed.

## Reproducible commands

```powershell
npm run build
cargo test --manifest-path src-tauri\Cargo.toml
.\.venv-build\Scripts\python.exe -m unittest discover -s backend/tests -v
cargo check --manifest-path src-tauri\Cargo.toml --bin audio_hardware_probe
```

Release validation:

```powershell
.\build_backend.bat
npm run tauri -- build
powershell -ExecutionPolicy Bypass -File .\build_release_zip.ps1
```

## Claims intentionally not made

- CPU, AMD, or Intel GPU inference is not claimed.
- Universal automatic input support for elevated windows, games, custom controls, or remote desktop is not claimed.
- Independent hardware-probe results are not presented as VoiceInput end-to-end transcription results.
- A successful source build is not presented as proof that the release ZIP was freshly extracted and verified.
