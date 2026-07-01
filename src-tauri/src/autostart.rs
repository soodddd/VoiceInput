//! VoiceInput v2 — 开机自启管理 (P2-05)
//!
//! 通过 Windows 注册表 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`
//! 实现开机自启。使用 `reg` 命令操作，无需额外依赖。
//!
//! - `enable()`  — 添加注册表项，开机自动启动 VoiceInput
//! - `disable()` — 移除注册表项
//! - `is_enabled()` — 检查是否已启用

use std::process::Command;

/// 注册表路径（当前用户开机自启项）
#[cfg(windows)]
const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const APP_NAME: &str = "VoiceInput";

/// 启用开机自启。
///
/// 将当前可执行文件路径写入注册表 Run 项。
pub fn enable() -> Result<(), String> {
    let exe_path = std::env::current_exe()
        .map_err(|e| format!("获取当前程序路径失败: {}", e))?;

    #[cfg(windows)]
    {
        let path_str = exe_path.to_string_lossy();
        // 注册表值需要用引号包裹路径（路径可能含空格）
        let value = format!("\"{}\"", path_str);

        let output = Command::new("reg")
            .args([
                "add",
                RUN_KEY,
                "/v",
                APP_NAME,
                "/t",
                "REG_SZ",
                "/d",
                &value,
                "/f",
            ])
            .output()
            .map_err(|e| format!("执行 reg 命令失败: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("注册表写入失败: {}", stderr));
        }

        log::info!("开机自启已启用: {}", path_str);
    }

    #[cfg(not(windows))]
    {
        let _ = exe_path;
        log::warn!("开机自启仅在 Windows 上支持");
    }

    Ok(())
}

/// 禁用开机自启。
///
/// 从注册表 Run 项中移除 VoiceInput 条目。
pub fn disable() -> Result<(), String> {
    #[cfg(windows)]
    {
        let output = Command::new("reg")
            .args(["delete", RUN_KEY, "/v", APP_NAME, "/f"])
            .output()
            .map_err(|e| format!("执行 reg 命令失败: {}", e))?;

        // reg delete 在值不存在时返回 1，但不影响功能
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            // 如果是因为值不存在，不算错误
            if !stderr.contains("Unable to find") && !stderr.contains("找不到") {
                return Err(format!("注册表删除失败: {}", stderr));
            }
            log::info!("开机自启项不存在，无需删除");
        } else {
            log::info!("开机自启已禁用");
        }
    }

    #[cfg(not(windows))]
    {
        log::warn!("开机自启仅在 Windows 上支持");
    }

    Ok(())
}

/// 检查开机自启是否已启用。
#[allow(dead_code)]
pub fn is_enabled() -> bool {
    #[cfg(windows)]
    {
        let output = match Command::new("reg")
            .args(["query", RUN_KEY, "/v", APP_NAME])
            .output()
        {
            Ok(o) => o,
            Err(_) => return false,
        };

        if !output.status.success() {
            return false;
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout.contains(APP_NAME)
    }

    #[cfg(not(windows))]
    {
        false
    }
}

/// 根据布尔值设置开机自启状态。
///
/// `true` → 启用，`false` → 禁用。
pub fn set_enabled(enabled: bool) -> Result<(), String> {
    if enabled {
        enable()
    } else {
        disable()
    }
}
