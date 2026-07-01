//! VoiceInput v2 — Rust 系统控制层
//!
//! 模块职责：
//! - `config` — 配置文件读写（%LOCALAPPDATA%\VoiceInput\config.json）
//! - `token` — UUID v4 token 生成与持久化
//! - `gpu_check` — NVIDIA GPU 检测（nvidia-smi）
//! - `hotkey` — 全局快捷键监听（rdev，独立线程）
//! - `recorder` — 麦克风录音（cpal / WASAPI）
//! - `paste` — 剪贴板写入 + SendInput Ctrl+V 粘贴（Windows API）
//! - `process_manager` — Python sidecar 进程管理
//! - `commands` — Tauri commands（前端可调用）
//! - `tray` — 系统托盘（Tauri 2 TrayIconBuilder）
//! - `audio_utils` — WAV 封装 + RMS 音量计算
//! - `errors` — 统一错误类型

mod audio_utils;
mod autostart;
mod commands;
mod config;
mod errors;
mod gpu_check;
mod hotkey;
mod paste;
mod process_manager;
mod recorder;
mod token;
mod tray;

use std::sync::{Arc, Mutex};
use tauri::Manager;

/// 应用全局共享状态。
///
/// 通过 Tauri 的 State 管理机制注入到各 command 中。
/// 所有字段都用 `Arc<Mutex<T>>` 包装，保证线程安全。
pub struct AppState {
    /// 应用配置
    pub config: Arc<Mutex<config::AppConfig>>,
    /// 录音器
    pub recorder: Arc<Mutex<recorder::Recorder>>,
    /// Python 后端进程管理器
    pub backend: Arc<Mutex<process_manager::BackendManager>>,
}

/// Tauri 应用入口点。
///
/// 启动流程：
/// 1. 初始化日志（输出到 stderr + 文件）
/// 2. GPU 检测（无 NVIDIA 显卡则 panic）
/// 3. 加载配置 + 生成 token
/// 4. 启动 Python sidecar 后端
/// 5. 启动全局快捷键监听
/// 6. 创建系统托盘
/// 7. 注册共享状态 + commands
/// 8. 启动后端健康监控线程（崩溃自动重启）
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 初始化日志（stderr + 文件）
    init_logging();

    log::info!("VoiceInput v2 启动中...");

    tauri::Builder::default()
        // 单实例锁必须第一个注册：第二个实例启动时触发回调并自动退出
        .plugin(tauri_plugin_single_instance::init(|app, _argv, _cwd| {
            log::info!("检测到第二个实例启动，聚焦主窗口");
            if let Some(window) = app.get_webview_window("floating") {
                let _ = window.show();
                let _ = window.set_focus();
            }
        }))
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // ── 1. GPU 检测 ──
            log::info!("[1/6] 检测 NVIDIA GPU...");
            if let Err(e) = gpu_check::check_nvidia_gpu() {
                log::error!("GPU 检测失败: {}", e);
                // 显示友好的弹窗提示，而非直接闪退
                show_gpu_error_dialog(&e);
                // 仍然退出（无 GPU 无法运行 ASR），但用户已知道原因
                #[cfg(not(debug_assertions))]
                {
                    std::process::exit(1);
                }
                #[cfg(debug_assertions)]
                log::warn!("开发模式: 跳过 GPU 检测失败退出");
            }

            // ── 2. 加载配置 + 生成 token ──
            log::info!("[2/6] 加载配置...");
            let mut cfg = config::load_config().map_err(|e| {
                log::error!("加载配置失败: {}", e);
                Box::<dyn std::error::Error>::from(e)
            })?;
            let token = token::ensure_token(&mut cfg);
            log::info!("当前 token: {}...", &token[..8.min(token.len())]);

            // ── 3. 启动 Python sidecar 后端 ──
            log::info!("[3/6] 启动 Python 后端...");
            let model_dir = get_model_dir();
            let server_port = 8765u16;
            let mut backend = process_manager::BackendManager::new(
                token.clone(),
                server_port,
                model_dir.to_string_lossy().to_string(),
                cfg.model_strategy.clone(),
            );

            if let Err(e) = backend.start() {
                log::error!("启动后端失败: {}", e);
                log::warn!("应用将继续启动，但语音功能不可用。请确认 asr_backend.exe 存在。");
            } else {
                log::info!("后端进程已启动，前端可通过 check_backend 轮询健康状态");
            }

            // 将 backend 包装为 Arc<Mutex<>> 以便共享给健康监控线程
            let backend = Arc::new(Mutex::new(backend));

            // ── 3.5 启动后端健康监控线程（崩溃自动重启） ──
            {
                let backend_clone = Arc::clone(&backend);
                std::thread::spawn(move || {
                    log::info!("后端健康监控线程已启动（每 10 秒检查一次）");
                    loop {
                        std::thread::sleep(std::time::Duration::from_secs(10));
                        let mut bm = match backend_clone.lock() {
                            Ok(guard) => guard,
                            Err(_) => continue,
                        };
                        if !bm.is_alive() {
                            log::warn!("后端进程未存活，尝试自动重启...");
                            match bm.restart() {
                                Ok(()) => log::info!("后端自动重启成功"),
                                Err(e) => log::error!("后端自动重启失败: {}", e),
                            }
                        }
                    }
                });
            }

            // ── 4. 启动快捷键监听 ──
            log::info!("[4/6] 启动快捷键监听...");
            let app_handle = app.handle().clone();
            let hotkey_config = cfg.clone();
            hotkey::start_hotkey_listener(app_handle, hotkey_config);

            // ── 5. 创建系统托盘 ──
            log::info!("[5/6] 创建系统托盘...");
            if let Err(e) = tray::create_tray(app.handle()) {
                log::error!("创建系统托盘失败: {}", e);
            }

            // ── 6. 注册共享状态 ──
            log::info!("[6/6] 注册应用状态...");
            let state = AppState {
                config: Arc::new(Mutex::new(cfg)),
                recorder: Arc::new(Mutex::new(recorder::Recorder::new())),
                backend,
            };
            app.manage(state);

            log::info!("VoiceInput v2 启动完成！");
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // 录音
            commands::start_recording,
            commands::stop_recording,
            commands::transcribe_and_paste,
            commands::paste_text,
            commands::get_devices,
            // 配置
            commands::get_config,
            commands::save_config,
            // 后端
            commands::check_backend,
            // 模型
            commands::get_model_status,
            commands::download_model,
            commands::load_model,
            commands::unload_model,
            commands::get_download_status,
            commands::cancel_download,
            // 模型策略 (P1-06)
            commands::get_model_strategy,
            commands::set_model_strategy,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 获取模型存储目录路径。
///
/// 路径: `%LOCALAPPDATA%\VoiceInput\models`
fn get_model_dir() -> std::path::PathBuf {
    config::get_config_dir().join("models")
}

/// 初始化日志系统：同时输出到 stderr 和日志文件。
///
/// 日志文件路径: `%LOCALAPPDATA%\VoiceInput\logs\voiceinput-YYYY-MM-DD.log`
fn init_logging() {
    let log_dir = config::get_config_dir().join("logs");
    let _ = std::fs::create_dir_all(&log_dir);

    let log_file = log_dir.join(format!(
        "voiceinput-{}.log",
        chrono::Local::now().format("%Y-%m-%d")
    ));

    let mut dispatch = fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "{} [{} {}] {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"),
                record.target(),
                record.level(),
                message
            ))
        })
        .level(log::LevelFilter::Info)
        .chain(std::io::stderr());

    // 尝试添加文件日志，若失败则仅使用 stderr
    match fern::log_file(&log_file) {
        Ok(f) => {
            dispatch = dispatch.chain(f);
        }
        Err(e) => {
            eprintln!("无法创建日志文件 {:?}: {}", log_file, e);
        }
    }

    dispatch.apply().expect("Failed to initialize logging");

    log::info!("日志文件: {}", log_file.display());
}

/// 显示 GPU 检测失败的友好弹窗。
///
/// 使用 Windows MessageBox API（无需应用窗口已创建），
/// 给出清晰的中文提示和解决建议。
#[cfg(windows)]
fn show_gpu_error_dialog(error: &str) {
    use windows::Win32::UI::WindowsAndMessaging::{
        MessageBoxW, MB_ICONERROR, MB_OK, MB_SETFOREGROUND, MB_TOPMOST,
    };
    use windows::core::PCWSTR;

    let title: Vec<u16> = "VoiceInput 无法启动\0"
        .encode_utf16()
        .collect();
    let message = format!(
        "抱歉，VoiceInput 无法启动。\n\n原因：{}\n\n解决方法：\n\
        1. 确认你的电脑有 NVIDIA 独立显卡\n\
        2. 安装最新的 NVIDIA 显卡驱动\n\
        3. 如果是笔记本电脑，请在 NVIDIA 控制面板中设置使用独立显卡\n\n\
        如需帮助，请访问 GitHub 仓库提交 issue。",
        error
    );
    let message_wide: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();

    unsafe {
        let _ = MessageBoxW(
            None,
            PCWSTR(message_wide.as_ptr()),
            PCWSTR(title.as_ptr()),
            MB_OK | MB_ICONERROR | MB_TOPMOST | MB_SETFOREGROUND,
        );
    }
}

#[cfg(not(windows))]
fn show_gpu_error_dialog(error: &str) {
    eprintln!("FATAL: {}", error);
}
