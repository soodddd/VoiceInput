//! VoiceInput v2 — Python Sidecar 进程管理
//!
//! 负责 asr_backend.exe 的启动、健康检查、停止和重启。
//! 使用 std::process::Command 管理子进程，Windows 下隐藏控制台窗口。

use std::process::{Child, Command, Stdio};
use std::time::Duration;

/// Python 后端进程管理器。
///
/// 管理 asr_backend.exe 的生命周期。一个 BackendManager 实例对应一个后端进程。
pub struct BackendManager {
    /// 子进程句柄
    child: Option<Child>,
    /// 鉴权 token
    token: String,
    /// 监听端口
    port: u16,
    /// 模型目录路径
    model_dir: String,
    /// 模型策略: "fast" / "balanced" / "accurate" / "memory"
    pub model_strategy: String,
    /// 服务地址（http://127.0.0.1:port）
    server_url: String,
}

/// Windows 下隐藏控制台窗口的标志位
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

impl BackendManager {
    /// 创建新的后端管理器。
    ///
    /// # 参数
    /// - `token`: 鉴权 token
    /// - `port`: 监听端口（默认 8765）
    /// - `model_dir`: 模型存储目录
    /// - `model_strategy`: 模型策略（"fast" / "balanced" / "accurate" / "memory"）
    pub fn new(token: String, port: u16, model_dir: String, model_strategy: String) -> Self {
        let server_url = format!("http://127.0.0.1:{}", port);
        BackendManager {
            child: None,
            token,
            port,
            model_dir,
            model_strategy,
            server_url,
        }
    }

    /// 返回服务地址。
    pub fn server_url(&self) -> &str {
        &self.server_url
    }

    /// 返回 token。
    pub fn token(&self) -> &str {
        &self.token
    }

    /// 启动 Python sidecar 后端进程。
    ///
    /// 命令: `asr_backend.exe --token <token> --port <port> --model-dir <dir> --device cuda:0 --model-strategy <strategy>`
    ///
    /// Windows 下使用 CREATE_NO_WINDOW 标志隐藏控制台窗口。
    /// stdout/stderr 重定向到日志文件（或继承父进程）。
    pub fn start(&mut self) -> Result<(), String> {
        if self.child.is_some() {
            log::warn!("后端进程已在运行");
            return Ok(());
        }

        log::info!(
            "启动后端进程: asr_backend.exe --token *** --port {} --model-dir {} --device cuda:0 --model-strategy {}",
            self.port,
            self.model_dir,
            self.model_strategy
        );

        // 查找 asr_backend.exe
        let exe_name = if cfg!(windows) {
            "asr_backend.exe"
        } else {
            "asr_backend"
        };

        // 尝试多个可能的路径
        let exe_path = self.find_backend_exe(exe_name)?;

        log::info!("后端可执行文件路径: {}", exe_path.display());

        let mut cmd = Command::new(&exe_path);
        cmd.args([
            "--token", &self.token,
            "--port", &self.port.to_string(),
            "--model-dir", &self.model_dir,
            "--device", "cuda:0",
            "--model-strategy", &self.model_strategy,
        ]);

        // Windows: 隐藏控制台窗口
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        // 重定向 stdout/stderr 到日志文件
        let log_dir = crate::config::get_config_dir().join("logs");
        let _ = std::fs::create_dir_all(&log_dir);
        let sidecar_log = log_dir.join(format!(
            "sidecar-{}.log",
            chrono::Local::now().format("%Y-%m-%d")
        ));
        let log_handle = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&sidecar_log)
            .map_err(|e| format!("创建 sidecar 日志文件失败: {}", e))?;
        let log_handle2 = log_handle
            .try_clone()
            .map_err(|e| format!("克隆日志文件句柄失败: {}", e))?;
        cmd.stdout(Stdio::from(log_handle));
        cmd.stderr(Stdio::from(log_handle2));

        let child = cmd
            .spawn()
            .map_err(|e| format!("启动后端进程失败: {}", e))?;

        self.child = Some(child);
        log::info!("后端进程已启动 (PID: {:?})", self.child.as_ref().unwrap().id());

        Ok(())
    }

    /// 查找 asr_backend.exe 的位置。
    ///
    /// Only trusted application/development directories are searched.  Falling
    /// back to PATH could execute an unrelated program with the same name.
    fn find_backend_exe(&self, exe_name: &str) -> Result<std::path::PathBuf, String> {
        let mut candidates = Vec::new();
        if let Ok(exe_dir) = std::env::current_exe() {
            if let Some(parent) = exe_dir.parent() {
                candidates.push(parent.join("asr_backend").join(exe_name));
                candidates.push(parent.join("binaries").join("asr_backend").join(exe_name));
                candidates.push(parent.join(exe_name));
            }
        }
        let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        candidates.push(manifest.join("binaries").join("asr_backend").join(exe_name));
        for candidate in candidates {
            if candidate.is_file() {
                return candidate
                    .canonicalize()
                    .map_err(|e| format!("无法解析后端路径: {}", e));
            }
        }
        Err("找不到受信任的 ASR 后端，请重新安装完整运行包".to_string())
    }

    /// 检查后端进程是否存活。
    ///
    /// 通过尝试 waitpid(non-blocking) 判断子进程状态。
    pub fn is_alive(&mut self) -> bool {
        let status = match self.child.as_mut() {
            Some(child) => child.try_wait(),
            None => return false,
        };
        match status {
            Ok(None) => true,
            Ok(Some(exit_status)) => {
                log::warn!("后端进程已退出: {}", exit_status);
                self.child = None;
                false
            }
            Err(e) => {
                log::error!("检查后端进程状态失败: {}", e);
                self.child = None;
                false
            }
        }
    }

    /// 停止后端进程。
    ///
    /// 先尝试优雅关闭（Windows: taskkill），再强制 kill。
    pub fn stop(&mut self) -> Result<(), String> {
        if let Some(mut child) = self.child.take() {
            log::info!("正在停止后端进程...");

            // Windows: 使用 taskkill 优雅终止进程树
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                let pid = child.id();
                let _ = Command::new("taskkill")
                    .args(["/PID", &pid.to_string(), "/T", "/F"])
                    .creation_flags(CREATE_NO_WINDOW)
                    .output();

                // 等待进程退出
                let _ = child.wait();
            }

            #[cfg(not(windows))]
            {
                // Unix: 发送 SIGTERM
                let _ = child.kill();
                let _ = child.wait();
            }

            log::info!("后端进程已停止");
        } else {
            log::info!("后端进程未运行，无需停止");
        }
        Ok(())
    }

    /// 重启后端进程。
    ///
    /// 先 stop 再 start；健康监控线程会在后续周期确认 HTTP 健康状态。
    pub fn restart(&mut self) -> Result<(), String> {
        log::info!("重启后端进程...");
        self.stop()?;
        // 短暂等待端口释放
        std::thread::sleep(Duration::from_millis(500));
        self.start()
    }
}

impl Drop for BackendManager {
    fn drop(&mut self) {
        if self.child.is_some() {
            let _ = self.stop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_manager() {
        let mut mgr = BackendManager::new(
            "test-token".to_string(),
            8765,
            "/tmp/models".to_string(),
            "balanced".to_string(),
        );
        assert_eq!(mgr.token(), "test-token");
        assert_eq!(mgr.server_url(), "http://127.0.0.1:8765");
        assert!(!mgr.is_alive());
    }
}
