# VoiceInput 0.1.4 升级指南（中文）

本指南用于从 0.1.3-preview 或更早版本升级到 0.1.4-preview。它同时适用于运行包用户和源码开发者。

## A. 使用 GitHub 运行包升级

1. 在旧版托盘菜单中选择退出；确认任务管理器中没有 `voiceinput.exe` 或 `asr_backend.exe`。
2. 下载 0.1.4 的完整 ZIP，解压到新的目录，例如 `C:\VoiceInput-0.1.4`。
3. 检查目录中同时存在：
   - `voiceinput.exe`
   - `asr_backend\asr_backend.exe`
   - `asr_backend\_internal\`
   - `resources\default_config.json`
   - `SHA256SUMS.txt`
4. 不要把新 `voiceinput.exe` 放进旧版 `asr_backend` 目录，也不要删除 `_internal`。
5. 启动新目录中的 `voiceinput.exe`。

运行包升级不会删除用户配置和模型缓存。配置和模型默认在：

- 配置：`%LOCALAPPDATA%\VoiceInput\config.json`
- 模型：`%LOCALAPPDATA%\VoiceInput\models\`
- 日志：`%LOCALAPPDATA%\VoiceInput\logs\`

## B. 升级后的首次验收

按顺序完成以下检查：

1. 打开“设置 → 麦克风”，确认选中的设备是当前正在使用的麦克风。
2. 点击“测试麦克风”，说话 2 秒，确认出现“麦克风正常”和非零音量。
3. 关闭设置，打开 Notepad，点击空白文本区。
4. 按住 `Alt+V` 说一句完整短句，松开快捷键。
5. 确认窗口依次经历录音、处理中、结果状态，并确认文字进入 Notepad。
6. 再重复一次，确认第二次录音不需要重启应用。
7. 如果启用了 VAD，停止说话并等待静音自动结束；如果不希望自动结束，可在设置里关闭 VAD。

## C. 从源码构建 0.1.4

在仓库根目录执行：

```powershell
npm install
python -m pip install -r backend\requirements.txt
python -m pip install pyinstaller==6.22.1
.\build_backend.bat
npm run build
npm run tauri -- build
powershell -ExecutionPolicy Bypass -File .\build_release_zip.ps1
```

构建顺序不能颠倒：先重建 Python sidecar，再重建 Tauri 主程序，最后生成 ZIP。这样可以避免桌面端与 sidecar 的版本或 API 不一致。

## D. 如何确认没有混入旧文件

```powershell
rg -n --hidden -g '!node_modules' -g '!src-tauri/target' -g '!release' '0\.1\.3|0\.1\.2' .
```

搜索结果中允许出现历史发布说明和依赖锁文件版本；以下位置不应再出现旧的应用版本：

- `package.json`、`package-lock.json`
- `src-tauri/Cargo.toml`、`src-tauri/tauri.conf.json`
- `backend/__main__.py`、`backend/server.py`
- `README.md`、`README.zh-CN.md`

启动 sidecar 后，日志应出现 `VoiceInput ASR backend v0.1.4`，健康检查应返回 `status=ok`。

## E. 音频故障定位

如果“录音开始”但识别为空：

1. 先在设置中做麦克风测试。
2. 再运行独立探针对照麦克风和蓝牙音箱：

```powershell
cargo run --manifest-path src-tauri\Cargo.toml --bin audio_hardware_probe -- test-artifacts\speech\chinese.wav
cargo run --manifest-path src-tauri\Cargo.toml --bin audio_hardware_probe -- test-artifacts\speech\chinese.wav --no-playback
```

3. 播放模式有明显 RMS、静音模式接近底噪，说明系统硬件和路由基本正常，应继续看 VoiceInput 日志中的 RMS/峰值和后端日志。
4. 两种模式都接近零，优先检查 Windows 隐私设置、默认输入/输出设备、蓝牙连接和麦克风静音。

## F. 回滚

保留旧版完整目录即可回滚。退出 0.1.4 后启动旧版 `voiceinput.exe`；不要把 0.1.4 的 `asr_backend` 与旧桌面端混用。模型缓存通常可以保留，如果旧版本无法读取，再为旧版指定独立模型目录。
