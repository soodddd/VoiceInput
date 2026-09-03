//! VoiceInput v2 — Token 生成与管理
//!
//! 生成随机 UUID v4 token，用于 Rust 层与 Python sidecar 之间的鉴权。
//! Token is generated for each application process and is never persisted or
//! exposed to the WebView.

use crate::config::AppConfig;
use uuid::Uuid;

/// 生成一个新的 UUID v4 token 字符串（无连字符的大写格式）。
///
/// 示例: `"A1B2C3D4E5F6..."`
pub fn generate_token() -> String {
    Uuid::new_v4().to_string().to_uppercase()
}

/// 确保配置中存在有效 token；若为空则生成新 token 并保存到磁盘。
///
/// # 参数
/// - `config`: 可变引用的 AppConfig，token 字段会被原地填充
///
/// # 返回
/// 当前的 token 字符串（clone）
pub fn ensure_token(config: &mut AppConfig) -> String {
    if config.token.is_none() || config.token.as_ref().map(|s| s.is_empty()).unwrap_or(true) {
        let new_token = generate_token();
        config.token = Some(new_token.clone());
        log::info!("已生成本次运行使用的本地鉴权 token");
        new_token
    } else {
        config.token.clone().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_token_length() {
        let t = generate_token();
        // UUID v4 带连字符是 36 字符，转大写仍 36
        assert_eq!(t.len(), 36);
        assert!(t.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '-'));
    }

    #[test]
    fn test_generate_token_unique() {
        let t1 = generate_token();
        let t2 = generate_token();
        assert_ne!(t1, t2);
    }
}
