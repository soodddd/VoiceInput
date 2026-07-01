//! VoiceInput v2 — 统一错误类型
//!
//! 所有模块的失败都归一化为 `AppError`，便于 Tauri command 层统一处理。
//! Tauri command 返回 `Result<T, String>`，通过 `format!("{:?}", e)` 转换。

use serde::Serialize;

/// 应用级错误枚举，覆盖所有子系统。
#[derive(Debug, Clone, Serialize)]
pub enum AppError {
    /// GPU 检测失败（无 NVIDIA 显卡 / nvidia-smi 不可用等）
    GpuCheck(String),
    /// 配置文件读写失败
    Config(String),
    /// 录音相关错误（设备打开、采样率不匹配等）
    Recording(String),
    /// Python sidecar 后端通信错误
    Backend(String),
    /// 剪贴板 / SendInput 粘贴失败
    Paste(String),
    /// 通用 IO 错误
    Io(String),
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::GpuCheck(msg) => write!(f, "GPU检测错误: {}", msg),
            AppError::Config(msg) => write!(f, "配置错误: {}", msg),
            AppError::Recording(msg) => write!(f, "录音错误: {}", msg),
            AppError::Backend(msg) => write!(f, "后端错误: {}", msg),
            AppError::Paste(msg) => write!(f, "粘贴错误: {}", msg),
            AppError::Io(msg) => write!(f, "IO错误: {}", msg),
        }
    }
}

impl std::error::Error for AppError {}

/// 允许 AppError 自动转换为 String，供 Tauri command 使用。
impl From<AppError> for String {
    fn from(e: AppError) -> String {
        format!("{:?}", e)
    }
}

/// 便捷宏：将任意 `std::io::Error` 转为 `AppError::Io`。
impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::Io(e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display() {
        let e = AppError::GpuCheck("no nvidia".into());
        assert!(format!("{}", e).contains("GPU检测错误"));
    }

    #[test]
    fn test_into_string() {
        let e = AppError::Config("bad json".into());
        let s: String = e.into();
        assert!(s.contains("Config"));
    }
}
