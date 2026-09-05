# VoiceInput 0.1.3 文档导航 / Documentation

应用版本为 **0.1.3**，发布阶段为 **preview**。项目名称中的 v2 是架构代际，不是应用发布版本号。

- [中文介绍与使用说明](../README.zh-CN.md)
- [English introduction and usage](../README.md)
- [0.1.3 发布说明、验证边界与分发状态](RELEASE_NOTES_v0.1.3-preview.md)
- [历史代码审查与修复记录](CODE_REVIEW_FIX_REPORT_2026-09-03.md)
- [产品需求基线](PRD.md)：包含规划，不等于已交付功能。
- [架构设计基线](ARCHITECTURE.md)：历史设计，以当前源码为准。
- [0.1.2 历史发布说明](RELEASE_NOTES_v0.1.2-preview.md)：保留原版本，不改写历史。

## 当前交付边界

GitHub Release 尚无可运行的二进制附件。源码版本配置已统一为 0.1.3；旧本地 ZIP 存在后端版本不一致记录，不能作为已验收的新版本。构建成功、服务健康检查、真实语音识别和目标窗口输入是不同验收项目。

Before distributing a runtime, rebuild both components, check their versions, fresh-extract the complete onedir package, and test microphone transcription and target-window input. Keep `asr_backend/_internal` beside the backend executable.
