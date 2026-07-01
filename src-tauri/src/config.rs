//! VoiceInput v2 — 配置管理
//!
//! 配置文件存储路径：`%LOCALAPPDATA%\VoiceInput\config.json`
//! 首次启动时从内嵌的 `resources/default_config.json` 读取默认值。
//! 配置结构体同时包含 token 字段（运行时生成，不写入默认配置模板）。

use crate::errors::AppError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

/// 应用配置结构体，对应 config.json 文件内容。
/// 字段命名与 resources/default_config.json 保持一致（snake_case）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 主快捷键，如 "alt+v"（按下开始录音，松开停止）
    pub hotkey: String,
    /// 语言切换快捷键，如 "alt+l"
    pub language_hotkey: String,
    /// 识别语言: "auto" / "Chinese" / "English"
    pub language: String,
    /// 采样率，默认 16000
    pub sample_rate: u32,
    /// 声道数，固定为 1（单声道）
    pub channels: u32,
    /// 粘贴前延迟（毫秒），等待焦点切换
    pub paste_delay_ms: u64,
    /// 粘贴后是否恢复原剪贴板内容
    pub clipboard_restore: bool,
    /// 输入设备索引（None = 系统默认设备）
    pub input_device: Option<i32>,
    /// 是否启用音频归一化
    pub normalize_audio: bool,
    /// 是否裁剪首尾静音
    pub trim_silence: bool,
    /// 静音阈值（dB），低于此值视为静音
    pub silence_threshold_db: i32,
    /// 最大录音时长（秒），防止无限录音
    pub max_record_sec: u32,
    /// HTTP 请求超时（秒）
    pub request_timeout_sec: u32,
    /// Python sidecar 服务地址
    pub server_url: String,
    /// 模型路径（None = 使用默认路径）
    pub model_path: Option<String>,
    /// 模型策略: "fast" / "balanced" / "accurate" / "memory"
    pub model_strategy: String,

    /// ── P2 功能字段 ──
    /// P2-05: 开机自启
    #[serde(default)]
    pub auto_start: bool,
    /// P2-06: 标点模式: "raw"（原始输出）/ "simple"（简单标点）/ "input_method"（输入法模式）
    #[serde(default = "default_punctuation_mode")]
    pub punctuation_mode: String,
    /// P2-07: 中英混排自动加空格
    #[serde(default = "default_true")]
    pub auto_space_zh_en: bool,
    /// P2-02: VAD 语音活动检测（静音自动停止）
    #[serde(default = "default_true")]
    pub vad_enabled: bool,

    /// ── 运行时字段（不写入 default_config.json 模板）──
    /// 鉴权 token，首次启动自动生成
    #[serde(default)]
    pub token: Option<String>,
    /// 当前选定的输入设备名（用于 sidecar 设备映射，可选）
    #[serde(default)]
    pub input_device_name: Option<String>,
    /// 用户自定义术语词典（ASR误识别 → 正确文本）
    #[serde(default)]
    pub custom_terms: HashMap<String, String>,
}

/// 默认标点模式
fn default_punctuation_mode() -> String {
    "simple".to_string()
}

/// 默认 true
fn default_true() -> bool {
    true
}

impl Default for AppConfig {
    /// 返回与 resources/default_config.json 一致的默认配置。
    fn default() -> Self {
        AppConfig {
            hotkey: "alt+v".to_string(),
            language_hotkey: "alt+l".to_string(),
            language: "auto".to_string(),
            sample_rate: 16000,
            channels: 1,
            paste_delay_ms: 800,
            clipboard_restore: true,
            input_device: None,
            normalize_audio: true,
            trim_silence: true,
            silence_threshold_db: -40,
            max_record_sec: 120,
            request_timeout_sec: 120,
            server_url: "http://127.0.0.1:8765".to_string(),
            model_path: None,
            model_strategy: "balanced".to_string(),
            auto_start: false,
            punctuation_mode: "simple".to_string(),
            auto_space_zh_en: true,
            vad_enabled: true,
            token: None,
            input_device_name: None,
            custom_terms: HashMap::new(),
        }
    }
}

/// 获取配置目录路径：`%LOCALAPPDATA%\VoiceInput\`
///
/// 在 Windows 上即 `C:\Users\<用户名>\AppData\Local\VoiceInput\`。
/// 若 LOCALAPPDATA 环境变量不存在，回退到用户主目录下的 `.voiceinput`。
pub fn get_config_dir() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            let home = dirs_fallback();
            home.join(".voiceinput")
        });
    base.join("VoiceInput")
}

/// 回退方案：获取用户主目录。
fn dirs_fallback() -> PathBuf {
    if let Ok(userprofile) = std::env::var("USERPROFILE") {
        return PathBuf::from(userprofile);
    }
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home);
    }
    PathBuf::from(".")
}

/// 获取配置文件完整路径。
pub fn get_config_path() -> PathBuf {
    get_config_dir().join("config.json")
}

/// 加载配置文件。
///
/// 流程：
/// 1. 尝试读取 `%LOCALAPPDATA%\VoiceInput\config.json`
/// 2. 若文件不存在，使用 `AppConfig::default()` 作为初始值，并保存到磁盘
/// 3. 若文件存在但解析失败，记录警告并回退到默认值
pub fn load_config() -> Result<AppConfig, AppError> {
    let config_path = get_config_path();
    log::info!("配置文件路径: {}", config_path.display());

    if !config_path.exists() {
        log::info!("配置文件不存在，使用默认配置并创建");
        let default_cfg = AppConfig::default();
        save_config(&default_cfg)?;
        return Ok(default_cfg);
    }

    let content = fs::read_to_string(&config_path)
        .map_err(|e| AppError::Config(format!("读取配置文件失败: {}", e)))?;

    match serde_json::from_str::<AppConfig>(&content) {
        Ok(cfg) => {
            log::info!("配置加载成功");
            Ok(cfg)
        }
        Err(e) => {
            log::warn!("配置文件解析失败 ({}), 回退到默认配置", e);
            let default_cfg = AppConfig::default();
            // 尝试覆盖损坏的配置文件
            let _ = save_config(&default_cfg);
            Ok(default_cfg)
        }
    }
}

/// 保存配置到磁盘。
///
/// 会自动创建配置目录（若不存在），并以格式化 JSON 写入。
pub fn save_config(cfg: &AppConfig) -> Result<(), AppError> {
    let config_dir = get_config_dir();
    fs::create_dir_all(&config_dir)
        .map_err(|e| AppError::Config(format!("创建配置目录失败: {}", e)))?;

    let json = serde_json::to_string_pretty(cfg)
        .map_err(|e| AppError::Config(format!("序列化配置失败: {}", e)))?;

    let config_path = get_config_path();
    fs::write(&config_path, json)
        .map_err(|e| AppError::Config(format!("写入配置文件失败: {}", e)))?;

    log::info!("配置已保存到 {}", config_path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let cfg = AppConfig::default();
        assert_eq!(cfg.hotkey, "alt+v");
        assert_eq!(cfg.sample_rate, 16000);
        assert_eq!(cfg.channels, 1);
        assert!(cfg.clipboard_restore);
        assert!(cfg.token.is_none());
    }

    #[test]
    fn test_config_dir_exists() {
        let dir = get_config_dir();
        // 路径应以 VoiceInput 结尾
        assert!(dir.to_string_lossy().ends_with("VoiceInput"));
    }

    #[test]
    fn test_serialize_deserialize() {
        let cfg = AppConfig::default();
        let json = serde_json::to_string(&cfg).unwrap();
        let cfg2: AppConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(cfg.hotkey, cfg2.hotkey);
        assert_eq!(cfg.sample_rate, cfg2.sample_rate);
    }
}
