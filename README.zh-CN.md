# VoiceInput

**v0.1.0-preview · Windows 本地语音输入法**

[English](./README.md) | [简体中文](./README.zh-CN.md)

> ⚠️ **预览版本（Preview）** — 这是用于测试和反馈的早期预览版本。在正式版发布前，可能存在 Bug 和不兼容变更。

一款隐私优先、完全本地的 Windows 语音输入工具。语音识别全部在你的 GPU 上通过 [Qwen3-ASR](https://github.com/QwenLM/Qwen3) 完成 —— 音频数据绝不会离开你的电脑。

## 功能特性

- 🎙️ **按住说话** — 按住 `Alt+V` 说话，松开自动识别并粘贴到光标位置
- 🌐 **语言切换** — `Alt+L` 在 Auto / 中文 / 英文 之间循环切换
- 🔒 **100% 离线** — 所有识别在本地 GPU 完成，零云端调用
- 📝 **自定义术语** — 定义 ASR 误识别 → 正确文本的映射，自动替换
- ⚙️ **设置面板** — 麦克风选择、快捷键自定义、音频参数调节
- 🩺 **自动恢复** — 后端崩溃自动重启 + 文件日志记录
- 🖥️ **系统托盘** — 快速访问设置、查看日志、退出程序

## 系统要求

| 要求 | 说明 |
|------|------|
| 操作系统 | Windows 10 1903+ / Windows 11 |
| 显卡 | NVIDIA CUDA 11.8+ 兼容，≥ 4 GB 显存 |
| 磁盘 | 约 200 MB 程序 + 约 1.2 GB 模型 |
| 内存 | 8 GB+ |
| 输入 | 麦克风 |

## 快速开始

1. **下载** 最新版 `VoiceInput-v0.1.0-preview-win64-preview.zip`（见 [Releases 发布页](../../releases)）
2. **解压** zip 到任意目录（如 `C:\Program Files\VoiceInput\`）
3. **运行** `voiceinput.exe`
4. **首次启动** — 程序会提示下载语音识别模型（约 1.2 GB，仅需一次）
5. **按住 `Alt+V`** 说话，松开 —— 你的语音将自动粘贴到光标位置

## 从源码构建

### 前置条件

- [Node.js](https://nodejs.org/) 18+ 及 npm
- [Rust](https://rustup.rs/)（stable 工具链）
- Python 3.10+，并安装后端依赖（`torch`、`fastapi`、`qwen_asr` 等）
- [Tauri CLI 2.x](https://tauri.app/)（`npm install -D @tauri-apps/cli`）

### 构建步骤

```bash
# 1. 安装前端依赖
npm install

# 2. 构建 Python ASR 后端（PyInstaller onefile → src-tauri/binaries/）
.\build_backend.bat

# 3. 开发模式运行
npm run tauri dev

# 4. 构建发布 zip 包（voiceinput.exe + sidecar + 资源文件）
powershell -ExecutionPolicy Bypass -File .\build_release_zip.ps1
```

发布包输出到 `.\release\VoiceInput-v0.1.0-preview-win64-preview.zip`。

> **关于 NSIS 安装包** — PyInstaller onefile 模式的 sidecar 约 2.7 GB（打包了 torch + transformers），超过 NSIS 的 mmap 限制。因此项目采用 zip 分发方式。详见 `build_release_zip.ps1`。

## 架构

```
voiceinput.exe (Tauri/Rust)  ──启动──▶  asr_backend.exe (Python/FastAPI)
        │                                        │
        ├─ cpal（音频采集）                       ├─ Qwen3-ASR-0.6B（GPU 推理）
        ├─ enigo（模拟粘贴）                      ├─ uvicorn（HTTP 服务 127.0.0.1:8765）
        ├─ rdev（全局快捷键）                     └─ postprocess（术语修正）
        └─ React 18（悬浮窗 UI）
```

| 层级 | 技术 | 关键文件 |
|------|------|----------|
| 前端 | React 18 + TypeScript + Tailwind | `src/` |
| 原生外壳 | Rust + Tauri 2 | `src-tauri/src/` |
| ASR 后端 | Python + FastAPI + PyTorch | `backend/` |

## 配置说明

用户配置文件位于 `%LOCALAPPDATA%\VoiceInput\config.json`。日志文件位于 `%LOCALAPPDATA%\VoiceInput\logs\`。

## 项目结构

```
voice-input-v2/
├─ src/                    # React 前端
├─ src-tauri/              # Rust + Tauri 原生外壳
│  ├─ src/                 #   Rust 源码
│  ├─ binaries/            #   PyInstaller sidecar 输出（已 gitignore）
│  └─ tauri.conf.json      #   Tauri 打包配置
├─ backend/                # Python ASR 后端
│  ├─ server.py            #   FastAPI 服务
│  ├─ model_manager.py     #   模型下载/加载
│  ├─ postprocess.py       #   文本后处理
│  └─ asr_backend.spec     #   PyInstaller 打包配置
├─ resources/              # 默认配置、图标
├─ build_backend.bat       # 构建 sidecar EXE
├─ build_release_zip.ps1   # 构建发布 zip
└─ docs/PRD.md             # 产品需求文档
```

## 开源协议

MIT — 详见 [LICENSE](./LICENSE)。

## 致谢

- [Qwen3-ASR](https://github.com/QwenLM/Qwen3) — 语音识别模型
- [Tauri](https://tauri.app/) — 应用框架
- [PyInstaller](https://pyinstaller.org/) — Python 打包工具
