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
#[serde(default)]
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
    #[serde(skip_serializing, skip_deserializing, default = "default_server_url")]
    pub server_url: String,
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
    #[serde(skip_serializing, skip_deserializing, default)]
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

fn default_server_url() -> String {
    "http://127.0.0.1:8765".to_string()
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
            input_device: None,
            normalize_audio: true,
            trim_silence: true,
            silence_threshold_db: -40,
            max_record_sec: 120,
            request_timeout_sec: 120,
            server_url: "http://127.0.0.1:8765".to_string(),
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
            if let Err(error) = validate_config(&cfg) {
                log::warn!("配置值无效 ({}), 使用安全默认配置", error);
                return Ok(AppConfig::default());
            }
            log::info!("配置加载成功");
            Ok(cfg)
        }
        Err(e) => {
            log::warn!("配置文件解析失败 ({}), 回退到默认配置", e);
            let corrupt_path = config_path.with_extension(format!(
                "corrupt-{}.json",
                chrono::Local::now().format("%Y%m%d-%H%M%S")
            ));
            if let Err(copy_error) = fs::copy(&config_path, &corrupt_path) {
                log::warn!("备份损坏配置失败: {}", copy_error);
            } else {
                log::warn!("损坏配置已备份到 {}", corrupt_path.display());
            }
            let default_cfg = AppConfig::default();
            let _ = save_config(&default_cfg);
            Ok(default_cfg)
        }
    }
}

/// 保存配置到磁盘。
///
/// 会自动创建配置目录（若不存在），并以格式化 JSON 写入。
pub fn save_config(cfg: &AppConfig) -> Result<(), AppError> {
    validate_config(cfg)?;
    let config_dir = get_config_dir();
    fs::create_dir_all(&config_dir)
        .map_err(|e| AppError::Config(format!("创建配置目录失败: {}", e)))?;

    let json = serde_json::to_string_pretty(cfg)
        .map_err(|e| AppError::Config(format!("序列化配置失败: {}", e)))?;

    let config_path = get_config_path();
    if config_path.exists() {
        let backup_path = config_path.with_extension("json.bak");
        fs::copy(&config_path, &backup_path)
            .map_err(|e| AppError::Config(format!("备份配置文件失败: {}", e)))?;
    }
    fs::write(&config_path, json)
        .map_err(|e| AppError::Config(format!("写入配置文件失败: {}", e)))?;

    log::info!("配置已保存到 {}", config_path.display());
    Ok(())
}

/// Reject settings that can break capture, hang requests, or route private
/// audio away from the loopback sidecar.
pub fn validate_config(cfg: &AppConfig) -> Result<(), AppError> {
    crate::hotkey::validate_hotkey(&cfg.hotkey)
        .map_err(|error| AppError::Config(format!("主快捷键无效: {}", error)))?;
    crate::hotkey::validate_hotkey(&cfg.language_hotkey)
        .map_err(|error| AppError::Config(format!("语言快捷键无效: {}", error)))?;
    if cfg.hotkey.eq_ignore_ascii_case(&cfg.language_hotkey) {
        return Err(AppError::Config("两个快捷键不能相同".to_string()));
    }
    if !(8_000..=192_000).contains(&cfg.sample_rate) {
        return Err(AppError::Config("采样率必须在 8000-192000 Hz".to_string()));
    }
    if cfg.channels != 1 {
        return Err(AppError::Config("当前仅支持单声道输出".to_string()));
    }
    if !(1..=600).contains(&cfg.max_record_sec) {
        return Err(AppError::Config("最大录音时长必须在 1-600 秒".to_string()));
    }
    if !(5..=600).contains(&cfg.request_timeout_sec) {
        return Err(AppError::Config("请求超时必须在 5-600 秒".to_string()));
    }
    if !(-80..=-10).contains(&cfg.silence_threshold_db) {
        return Err(AppError::Config("静音阈值必须在 -80 到 -10 dB".to_string()));
    }
    if cfg.paste_delay_ms > 3000 {
        return Err(AppError::Config("输入延迟不能超过 3000 毫秒".to_string()));
    }
    if !matches!(cfg.language.as_str(), "auto" | "Chinese" | "English") {
        return Err(AppError::Config("识别语言配置无效".to_string()));
    }
    if !matches!(
        cfg.model_strategy.as_str(),
        "fast" | "balanced" | "accurate" | "memory"
    ) {
        return Err(AppError::Config("模型策略配置无效".to_string()));
    }
    if !matches!(
        cfg.punctuation_mode.as_str(),
        "raw" | "simple" | "input_method"
    ) {
        return Err(AppError::Config("标点模式配置无效".to_string()));
    }
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
