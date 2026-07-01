//! VoiceInput v2 — NVIDIA GPU 检测
//!
//! 通过调用 `nvidia-smi` 命令验证系统是否存在可用的 NVIDIA 显卡。
//! 若检测失败，返回明确的中文错误提示，指导用户安装驱动。

use std::process::Command;

/// 检测系统是否存在 NVIDIA 显卡。
///
/// 实现方式：执行 `nvidia-smi --query-gpu=name --format=csv,noheader`，
/// 若命令成功执行且 stdout 非空，则认为存在可用 GPU。
///
/// # 返回
/// - `Ok(())`: 检测到 NVIDIA 显卡
/// - `Err(String)`: 未检测到显卡或 nvidia-smi 不可用
pub fn check_nvidia_gpu() -> Result<(), String> {
    log::info!("正在检测 NVIDIA GPU...");

    let output = Command::new("nvidia-smi")
        .args([
            "--query-gpu=name",
            "--format=csv,noheader",
        ])
        .output();

    match output {
        Ok(out) => {
            if !out.status.success() {
                log::error!("nvidia-smi 返回非零退出码");
                return Err(
                    "未检测到 NVIDIA 显卡。本软件需要 NVIDIA 独立显卡才能运行。".to_string(),
                );
            }
            let stdout = String::from_utf8_lossy(&out.stdout);
            let gpu_name = stdout.trim().to_string();
            if gpu_name.is_empty() {
                log::error!("nvidia-smi 输出为空");
                return Err(
                    "未检测到 NVIDIA 显卡。本软件需要 NVIDIA 独立显卡才能运行。".to_string(),
                );
            }
            log::info!("检测到 NVIDIA GPU: {}", gpu_name);
            Ok(())
        }
        Err(e) => {
            log::error!("无法执行 nvidia-smi: {}", e);
            Err(
                "未检测到 NVIDIA 显卡。本软件需要 NVIDIA 独立显卡才能运行。请确认已安装 NVIDIA 驱动。"
                    .to_string(),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_gpu_returns_result() {
        // 这个测试取决于运行环境是否有 GPU，不断言具体结果
        let _ = check_nvidia_gpu();
    }
}
