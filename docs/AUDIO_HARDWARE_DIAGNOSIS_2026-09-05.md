# 独立音频硬件诊断（2026-09-05）

本测试绕过 VoiceInput，使用 `src-tauri/src/bin/audio_hardware_probe.rs`：先打开默认麦克风，再通过 Windows 默认输出播放固定 WAV，同时保存麦克风单声道捕获并统计电平。

## 结果

| 项目 | 结果 |
|---|---|
| 默认输出 | `耳机 (COLORFIRE CL500)` |
| 默认输入 | `麦克风阵列 (适用于数字麦克风的英特尔® 智音技术)` |
| 输入格式 | 48,000 Hz / 2 声道 / F32 |
| 播放时 RMS | `0.094655`（约 `-20.5 dB`） |
| 播放时峰值 | `0.993988`（约 `-0.1 dB`） |
| 播放时高于 −40 dB 的样本 | `217,364`（20.76%） |
| 不播放时 RMS | `0.000006`（约 `-104.2 dB`） |
| 不播放时峰值 | `0.000336`（约 `-69.5 dB`） |

## 判断

蓝牙音箱已被系统选为默认输出，播放声能被默认麦克风清晰采集；不播放时噪声极低。因此当前硬件、蓝牙输出路由和麦克风本身均正常，VoiceInput 的 `-40 dB` VAD 门限对这条声学链路足够低。

上一轮 VoiceInput 的失败测试不能单独证明硬件故障：播放进程是由外部命令提前异步启动的，和录音按钮没有处在同一个受控测试进程内。下一步应在 VoiceInput 已进入录音态后，由同一测试流程启动播放，并检查其 RMS/识别结果。

## 可复现命令

```powershell
cargo run --quiet --manifest-path src-tauri/Cargo.toml --bin audio_hardware_probe -- `
  "E:\ASK\voice-input-v2\test-artifacts\speech\chinese.wav" `
  "E:\ASK\voice-input-v2\test-artifacts\audio_probe_chinese.wav"

cargo run --quiet --manifest-path src-tauri/Cargo.toml --bin audio_hardware_probe -- `
  "E:\ASK\voice-input-v2\test-artifacts\speech\chinese.wav" `
  "E:\ASK\voice-input-v2\test-artifacts\audio_probe_silence.wav" --no-playback
```

捕获的 WAV 和语音素材位于被 `.gitignore` 忽略的 `test-artifacts/`，不进入发布包。
