# VoiceInput 0.1.4-preview 验证报告（中文）

日期：2026-09-05
仓库：`E:\ASK\voice-input-v2`
分支：`master`

## 结论

0.1.4 已完成源码级、构建级、硬件对照级和用户现场可用性验证。当前可以作为 Preview 发布。验证结果证明：麦克风、蓝牙输出和独立 `cpal` 采集链路正常；应用录音生命周期经过工作线程隔离后可用于实际语音输入；用户现场截图显示识别完成、结果展示和目标窗口输入均已发生。

“Preview”仍然表示不同权限级别、不同输入控件和不同音频驱动组合没有全部认证，不表示当前功能不可用。

## 验证矩阵

| 层级 | 验证项 | 结果 | 证据/命令 |
|---|---|---|---|
| 前端 | TypeScript + Vite 构建 | PASS | `npm run build` |
| Rust | 单元测试 | PASS | `cargo test --manifest-path src-tauri/Cargo.toml` |
| Python | 后端测试 | PASS | `.venv-build\\Scripts\\python.exe -m unittest discover -s backend/tests -v` |
| 诊断工具 | `audio_hardware_probe` 编译 | PASS | `cargo check --manifest-path src-tauri/Cargo.toml --bin audio_hardware_probe` |
| sidecar | 版本/启动日志 | PASS | `Starting VoiceInput ASR backend v0.1.4` |
| 音频硬件 | 蓝牙播放 → 麦克风采集 | PASS | 播放约 `-20 dB RMS`，峰值约 `-0.1 dB` |
| 音频基线 | 不播放时采集 | PASS | 约 `-104 dB RMS`，超过 -40 dB 的采样为 0 |
| 桌面端 | 真实录音、识别、结果显示 | PASS | 用户现场 GUI 截图：识别完成并显示中文结果 |
| 目标输入 | 自动进入目标文本窗口 | PASS | 用户现场 GUI 截图：结果进入目标窗口 |
| 发布包 | fresh extraction + `_internal` + `/health` | PASS | `release/VoiceInput-v0.1.4-preview-win64.zip` |

## 本次桌面端实测记录

在同一台 Windows 验证机上，从 `src-tauri\target\release\voiceinput.exe` 启动 0.1.4，确认窗口标题为 `VoiceInput 0.1.4`，从本地缓存加载 Qwen3-ASR-0.6B 后执行一次端到端测试：

1. 打开 Notepad 并聚焦文本编辑区。
2. 点击 VoiceInput 麦克风按钮开始录音。
3. 通过默认扬声器播放仓库内 `test-artifacts\speech\chinese.wav`。
4. 录音自动结束后等待本地 GPU 推理。
5. 检查 VoiceInput 结果气泡和 Notepad 目标窗口。

实测日志摘要：

```text
设备：麦克风阵列 (适用于数字麦克风的英特尔® 智音技术)
配置：48000 Hz / 2 channels / F32
录音：约 4.8 s，RMS=0.034215，Peak=0.855103
识别：14 字符，处理耗时约 2269 ms，音频有效时长约 3070 ms
输入：已输入识别结果（14 字符）
sidecar：POST /transcribe -> 200 OK，/health.version=0.1.4
```

结果显示在 VoiceInput 窗口，并成功进入 Notepad；这条记录与用户提供的现场截图相互独立，但结论一致：当前版本可以完成“录音 → 本地识别 → 目标窗口输入”。

## 音频硬件对照

本机默认设备：

- 输出：蓝牙耳机/音箱 `耳机 (COLORFIRE CL500)`
- 输入：`麦克风阵列 (适用于数字麦克风的英特尔® 智音技术)`
- 输入格式：`48000 Hz / 2 channels / F32`

独立探针播放固定 WAV 并采集 12 秒，得到：

```text
播放模式：RMS=0.094655（约 -20.5 dB），Peak=0.993988（约 -0.1 dB）
播放模式：超过 -40 dB 的采样 217364，占 20.76%
静音基线：RMS=0.000006（约 -104.2 dB），Peak=0.000336（约 -69.5 dB）
静音基线：超过 -40 dB 的采样 0，占 0.00%
```

随后在 VoiceInput 录音期间并发运行独立探针，播放模式仍约为 `-20.8 dB RMS`，说明蓝牙输出和麦克风输入不会因为 VoiceInput 运行而失效。该结果把排查范围收敛到应用内部录音生命周期，而不是继续更换硬件。

## 发布包凭据

本次同批次生成的 ZIP 已完成全新目录解压验证和 sidecar `/health` 版本验证：

```text
文件：VoiceInput-v0.1.4-preview-win64.zip
大小：2,727.5 MB（2,859,943,059 bytes）
SHA-256：592E2B6A8F196AC04E0DF1AF162FBEB1FD1A62F04CF723719047B2B3D9B69E4E
验证：fresh extraction / asr_backend/_internal / SHA256SUMS / /health.version=0.1.4
```

由于完整 ZIP 超过 GitHub Release 单资产限制，本文件保留本地发布凭据；GitHub Release 发布源码和文档，不上传可能被截断的运行包。

## 0.1.4 代码重点验证

### 录音线程

`src-tauri/src/recorder.rs` 中一次会话由专用线程创建和持有 `cpal::Stream`，停止时先设置原子停止标志、等待线程结束，再读取缓冲区。这样保证 WASAPI 流在创建它的线程上释放，并避免启动失败时留下分离线程。

### 音量反馈

实时回调不再同步调用 WebView 的 `audio-level` 事件。回调写入原子 RMS 快照，独立采样线程每 50 ms 发出事件；前端 `useRecorder` 和设置页的麦克风测试仍使用原有事件协议，无需额外配置迁移。

### VAD

VAD 的 `record_start` 和 `last_sound_time` 在 `stream.play()` 成功后重置，避免设备初始化耗时被计入静音时间。停止日志同时记录总 RMS、峰值和 VAD 的最高 RMS。

## 可重复验证命令

```powershell
npm run build
cargo test --manifest-path src-tauri\Cargo.toml
.\.venv-build\Scripts\python.exe -m unittest discover -s backend/tests -v
cargo check --manifest-path src-tauri\Cargo.toml --bin audio_hardware_probe
```

发布验证：

```powershell
.\build_backend.bat
npm run tauri -- build
powershell -ExecutionPolicy Bypass -File .\build_release_zip.ps1
```

## 尚未宣称的内容

- 未宣称 CPU、AMD 或 Intel GPU 能够运行 Qwen3-ASR。
- 未宣称管理员窗口、游戏、自绘控件、远程桌面等全部支持自动输入。
- 未把独立硬件探针的结果冒充成 VoiceInput 全链路识别结果。
- 未把源码构建成功冒充成发布 ZIP 已被全新解压验证。
