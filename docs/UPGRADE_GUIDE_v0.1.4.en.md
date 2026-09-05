# VoiceInput 0.1.4 Upgrade Guide (English)

Use this guide to upgrade from 0.1.3-preview or earlier to 0.1.4-preview. It covers both runtime-package users and source developers.

## A. Upgrade from the GitHub runtime package

1. Exit the old instance from the tray menu and confirm that `voiceinput.exe` and `asr_backend.exe` are not still running.
2. Download the complete 0.1.4 ZIP and extract it to a new directory, for example `C:\VoiceInput-0.1.4`.
3. Check that the directory contains all of the following:
   - `voiceinput.exe`
   - `asr_backend\asr_backend.exe`
   - `asr_backend\_internal\`
   - `resources\default_config.json`
   - `SHA256SUMS.txt`
4. Do not put the new `voiceinput.exe` beside an old `asr_backend`, and do not remove `_internal`.
5. Launch `voiceinput.exe` from the new directory.

An upgrade does not delete user settings or the model cache. The default locations are:

- Settings: `%LOCALAPPDATA%\VoiceInput\config.json`
- Models: `%LOCALAPPDATA%\VoiceInput\models\`
- Logs: `%LOCALAPPDATA%\VoiceInput\logs\`

## B. First acceptance after the upgrade

Complete these checks in order:

1. Open Settings → Microphone and confirm that the selected device is the microphone you want to use.
2. Click Test Microphone, speak for two seconds, and confirm a normal result with non-zero level.
3. Close Settings, open Notepad, and focus its blank text area.
4. Hold `Alt+V`, speak one complete sentence, and release the hotkey.
5. Confirm the recording → processing → result states and confirm that text appears in Notepad.
6. Repeat the operation once more without restarting the app.
7. If VAD is enabled, stop speaking and wait for automatic silence completion; disable VAD in Settings if manual stop is preferred.

## C. Build 0.1.4 from source

Run these commands from the repository root:

```powershell
npm install
python -m pip install -r backend\requirements.txt
python -m pip install pyinstaller==6.22.1
.\build_backend.bat
npm run build
npm run tauri -- build
powershell -ExecutionPolicy Bypass -File .\build_release_zip.ps1
```

Keep the order: rebuild the Python sidecar, rebuild the Tauri desktop app, then create the ZIP. This prevents desktop/sidecar version and API mismatches.

## D. Check for stale application files

```powershell
rg -n --hidden -g '!node_modules' -g '!src-tauri/target' -g '!release' '0\.1\.3|0\.1\.2' .
```

Historical release notes and dependency lockfile versions may legitimately appear in the result. The following application entry points must not contain an old application version:

- `package.json`, `package-lock.json`
- `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`
- `backend/__main__.py`, `backend/server.py`
- `README.md`, `README.zh-CN.md`

After starting the sidecar, its log should contain `VoiceInput ASR backend v0.1.4`, and the health endpoint should return `status=ok`.

## E. Diagnose audio failures

If recording starts but recognition is empty:

1. Run the microphone test in Settings first.
2. Run the independent probe to compare the microphone and Bluetooth speaker:

```powershell
cargo run --manifest-path src-tauri\Cargo.toml --bin audio_hardware_probe -- test-artifacts\speech\chinese.wav
cargo run --manifest-path src-tauri\Cargo.toml --bin audio_hardware_probe -- test-artifacts\speech\chinese.wav --no-playback
```

3. A clear RMS level in playback mode and a near-noise-floor level in silence mode indicate that the system route and hardware are basically working. Continue with VoiceInput RMS/peak logs and the backend log.
4. If both modes are near zero, check Windows privacy permissions, default input/output devices, Bluetooth connectivity, and microphone mute state first.

## F. Rollback

Keep the previous complete directory for rollback. Exit 0.1.4 and launch the old `voiceinput.exe`; do not mix the 0.1.4 `asr_backend` with the old desktop executable. The model cache can normally be kept. If the old version cannot read it, give the old version a separate model directory.
