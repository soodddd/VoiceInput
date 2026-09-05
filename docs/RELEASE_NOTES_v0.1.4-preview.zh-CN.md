# VoiceInput v0.1.4-preview

发布日期：2026-09-05
应用版本：**0.1.4**
发布通道：**Preview**

## 一句话说明

0.1.4 是 VoiceInput 从“功能代码已经齐全”走向“可以反复使用并能够按同一流程发布”的版本。它保留完全本地的 Qwen3-ASR 识别能力，重点修复 Windows WASAPI 录音生命周期、音量反馈和版本/文档一致性。

## 本次升级内容

### 1. 录音链路更稳定

- `cpal`/WASAPI 音频流在专用工作线程创建、持有和释放，避免 Tauri 命令线程和 Windows 音频会话之间的生命周期冲突。
- `start_recording` 会等待音频流真正准备完成后再返回；启动失败或超时会等待工作线程退出，不遗留悬挂录音线程。
- 继续支持默认麦克风、按设备索引选择和按设备名称选择；采用设备的完整默认格式，Python 后端再完成最终 16 kHz 重采样。
- 录音缓冲区仍有最大时长上限，防止长时间录音无限增长。

### 2. 音量条和 VAD 更顺畅

- 实时音频回调只负责样本转换、限长缓存、RMS 计算和原子快照，不再从实时回调同步跨入 WebView。
- 独立采样线程每 50 ms 推送 `audio-level`，因此悬浮窗波形和“设置 → 麦克风 → 测试”仍能得到稳定反馈。
- VAD 的开始时间从音频流启动后计算，不会把 WASAPI 初始化耗时误算成静音。
- 保留最短录音时长、连续静音判定和最大录音时长保护。
- 停止日志记录总 RMS 与峰值；VAD 日志记录当前 RMS 与本次录音最高 RMS，便于区分“没有采到声音”和“模型没有识别出来”。

### 3. 增加硬件诊断工具

新增 `audio_hardware_probe` 开发工具，可以：

- 列出默认输入设备、默认输出设备和实际输入配置；
- 播放 WAV 到默认扬声器/蓝牙音箱，同时采集默认麦克风；
- 输出采样数量、RMS、峰值和超过阈值的采样比例；
- 使用 `--no-playback` 建立静音基线，帮助判断问题在扬声器、麦克风、系统路由还是 VoiceInput 内部。

它不是日常启动入口，也不会替代 VoiceInput 的识别流程。

### 4. 版本和发布一致性

以下入口统一为 `0.1.4`：

- `package.json` / `package-lock.json`
- `src-tauri/Cargo.toml` / `src-tauri/Cargo.lock`
- `src-tauri/tauri.conf.json`
- `backend/__main__.py` 启动日志
- `backend/server.py` FastAPI 元数据
- `index.html`、中英文 README、文档导航

发布脚本仍然使用 PyInstaller onedir 后端，并验证：完整解压、`asr_backend/_internal` 相对位置、SHA-256 清单和新解压 sidecar 的 `/health`。

本次同批次发布包已验证通过：`VoiceInput-v0.1.4-preview-win64.zip`，大小约 2,727.5 MB，SHA-256 为 `592E2B6A8F196AC04E0DF1AF162FBEB1FD1A62F04CF723719047B2B3D9B69E4E`。由于完整 ZIP 超过 GitHub Release 单资产限制，GitHub Release 不上传该大文件；源码、双语文档和可重复构建流程会正常发布。

## 从 0.1.3 升级

1. 退出正在运行的旧版 VoiceInput。
2. 完整替换 `voiceinput.exe` 和 `asr_backend` 目录；不要只替换其中一个文件。
3. 保留 `%LOCALAPPDATA%\VoiceInput\models`，已有模型缓存可以继续使用。
4. 启动后先打开“设置 → 麦克风 → 测试”。
5. 在 Notepad 或浏览器普通文本框中点击光标，按住 `Alt+V` 说一句短话，松开后确认识别结果和自动输入。

配置文件位置仍是 `%LOCALAPPDATA%\VoiceInput\config.json`，日志位置仍是 `%LOCALAPPDATA%\VoiceInput\logs\`。

## 验证状态

### 自动化验证

- 前端 TypeScript/Vite 构建：通过
- Rust 单元测试：通过
- Python 后端测试：通过
- 发布构建：要求桌面端和 sidecar 同批次重建
- 全新解压验证：要求保留 onedir `_internal` 并通过 `/health`

### 本机与现场验证

- 默认输出为蓝牙耳机/音箱，默认输入为 Intel 智音麦克风阵列；独立探针能采到约 `-20 dB RMS` 的播放声，静音基线约 `-104 dB RMS`。
- 用户现场 GUI 已观察到“识别完成”、识别文字显示并进入目标输入窗口，截图作为本次可用性证据。
- 不同管理员权限窗口、游戏、自绘控件和远程桌面环境仍需按目标程序单独验收。

## 已知边界

- 只支持 Windows 10 1903+ / Windows 11 和 NVIDIA CUDA GPU；不提供 CPU、AMD 或 Intel GPU 推理路径。
- 运行包约 2.7 GB，模型缓存约 1.9 GB；运行包和模型需要分别准备空间。
- GitHub 源码压缩包不是可直接运行的 onedir 包；运行时必须完整解压。
- 自动输入依赖目标程序接受 Windows 输入事件；如果目标程序以管理员权限运行，VoiceInput 也可能需要相同权限。
- Preview 不等于所有硬件、所有窗口类型都已经认证。

## 回滚

如果 0.1.4 在某个目标程序上出现回归，退出 VoiceInput，恢复上一版完整的 `voiceinput.exe` 与 `asr_backend` 目录即可；不要混用不同版本的桌面端和 sidecar。模型缓存通常无需回滚。
