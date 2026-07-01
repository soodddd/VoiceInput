//! VoiceInput v2 — Tauri Commands
//!
//! 所有前端可调用的 Tauri command 定义在此。
//! 共享状态通过 `State<AppState>` 注入（AppState 定义在 lib.rs）。
//! HTTP 请求使用 reqwest，与 Python sidecar 通信。

use crate::config::AppConfig;
use crate::paste;
use crate::recorder::AudioDeviceInfo;
use crate::AppState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::{AppHandle, Emitter, State};

/// /transcribe 接口返回的 JSON 结构
///
/// 注意：
/// - duration_ms 和 process_ms 使用 f64 而非 u64，因为 Python 后端
///   返回的是浮点数（如 6229.208946），serde 无法将带小数的浮点数反序列化为 u64。
/// - language 使用 Option<String>，因为后端在无法检测语言时返回 null，
///   serde 无法将 null 反序列化为 String。
#[derive(Debug, Deserialize, Serialize)]
pub struct TranscribeResponse {
    pub text: String,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub duration_ms: f64,
    #[serde(default)]
    pub process_ms: f64,
    #[serde(default)]
    pub chunks: serde_json::Value,
}

/// /model/status 接口返回的 JSON 结构
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ModelStatus {
    #[serde(default)]
    pub loaded: bool,
    #[serde(default)]
    pub downloading: bool,
    #[serde(default)]
    pub download_progress: f64,
    #[serde(default)]
    pub model_name: String,
    #[serde(default)]
    pub device: String,
    #[serde(default)]
    pub strategy: Option<String>,
}

/// /model/download/status 接口返回的 JSON 结构
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct DownloadStatus {
    #[serde(default)]
    pub downloading: bool,
    #[serde(default)]
    pub progress: f64,
    #[serde(default)]
    pub message: Option<String>,
}

// ──────────────────────────────────────────────
// 录音相关 commands
// ──────────────────────────────────────────────

/// 开始录音。
#[tauri::command]
pub fn start_recording(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let (sample_rate, device) = {
        let cfg = state
            .config
            .lock()
            .map_err(|e| format!("锁配置失败: {}", e))?;
        (cfg.sample_rate, cfg.input_device)
    };

    let mut recorder = state
        .recorder
        .lock()
        .map_err(|e| format!("锁录音器失败: {}", e))?;
    recorder.start(app, device, sample_rate).map_err(|e| {
        log::error!("开始录音失败: {}", e);
        e
    })
}

/// 停止录音，返回 WAV 字节流。
#[tauri::command]
pub fn stop_recording(state: State<'_, AppState>) -> Result<Vec<u8>, String> {
    let mut recorder = state
        .recorder
        .lock()
        .map_err(|e| format!("锁录音器失败: {}", e))?;
    recorder.stop().map_err(|e| {
        log::error!("停止录音失败: {}", e);
        e
    })
}

/// 转录并粘贴。
///
/// 流程：
/// 1. POST {server_url}/transcribe (multipart: audio=WAV, language)
/// 2. 解析返回的 JSON 获取 text
/// 3. 调用 paste::paste_text
/// 4. emit "transcribe-result" 事件
/// 5. 返回 text
#[tauri::command]
pub async fn transcribe_and_paste(
    app: AppHandle,
    state: State<'_, AppState>,
    wav: Vec<u8>,
    language: Option<String>,
    custom_terms: Option<HashMap<String, String>>,
) -> Result<String, String> {
    let (server_url, token, paste_delay, clipboard_restore, timeout_sec, config_terms) = {
        let cfg = state
            .config
            .lock()
            .map_err(|e| format!("锁配置失败: {}", e))?;
        (
            cfg.server_url.clone(),
            cfg.token.clone().unwrap_or_default(),
            cfg.paste_delay_ms,
            cfg.clipboard_restore,
            cfg.request_timeout_sec,
            cfg.custom_terms.clone(),
        )
    };

    // 合并传入的自定义术语和配置中的自定义术语
    let mut all_terms = config_terms;
    if let Some(extra) = custom_terms {
        for (k, v) in extra {
            all_terms.insert(k, v);
        }
    }

    log::info!(
        "开始转录: WAV {} bytes, language={:?}",
        wav.len(),
        language
    );

    // 构造 multipart form
    let part = reqwest::multipart::Part::bytes(wav)
        .file_name("audio.wav")
        .mime_str("audio/wav")
        .map_err(|e| format!("构造 multipart 失败: {}", e))?;

    let mut form = reqwest::multipart::Form::new().part("audio", part);
    if let Some(lang) = &language {
        form = form.text("language", lang.clone());
    }
    // 传递自定义术语到后端
    if !all_terms.is_empty() {
        let terms_json = serde_json::to_string(&all_terms)
            .map_err(|e| format!("序列化自定义术语失败: {}", e))?;
        form = form.text("custom_terms", terms_json);
    }

    let url = format!("{}/transcribe", server_url);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_sec as u64))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let response = client
        .post(&url)
        .header("X-VoiceInput-Token", &token)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("转录请求失败: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("转录请求返回错误: {} - {}", status, body));
    }

    let result: TranscribeResponse = response
        .json()
        .await
        .map_err(|e| format!("解析转录结果失败: {}", e))?;

    log::info!(
        "转录完成: text='{}' (耗时 {}ms, 音频 {}ms)",
        result.text,
        result.process_ms,
        result.duration_ms
    );

    // 粘贴文本
    if !result.text.is_empty() {
        paste::paste_text(result.text.clone(), paste_delay, clipboard_restore).map_err(|e| {
            log::error!("粘贴失败: {}", e);
            e
        })?;
    }

    // emit 事件
    let _ = app.emit("transcribe-result", &result.text);

    Ok(result.text)
}

/// 粘贴指定文本（写剪贴板 + 模拟 Ctrl+V）。
///
/// 用于前端在不经过转录流程的情况下手动粘贴文本。
#[tauri::command]
pub fn paste_text(state: State<'_, AppState>, text: String) -> Result<(), String> {
    let (paste_delay_ms, clipboard_restore) = {
        let cfg = state
            .config
            .lock()
            .map_err(|e| format!("锁配置失败: {}", e))?;
        (cfg.paste_delay_ms, cfg.clipboard_restore)
    };

    paste::paste_text(text, paste_delay_ms, clipboard_restore).map_err(|e| {
        log::error!("粘贴失败: {}", e);
        e
    })
}

/// 获取可用的音频输入设备列表。
#[tauri::command]
pub fn get_devices() -> Result<Vec<AudioDeviceInfo>, String> {
    Ok(crate::recorder::list_input_devices())
}

// ──────────────────────────────────────────────
// 配置相关 commands
// ──────────────────────────────────────────────

/// 获取当前配置的克隆。
#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<AppConfig, String> {
    let cfg = state
        .config
        .lock()
        .map_err(|e| format!("锁配置失败: {}", e))?;
    Ok(cfg.clone())
}

/// 保存配置。
///
/// 保存后调用 `hotkey::restart_hotkey_listener` 应用新的快捷键配置（热重载）。
#[tauri::command]
pub fn save_config(app: AppHandle, state: State<'_, AppState>, config: AppConfig) -> Result<(), String> {
    let mut cfg = state
        .config
        .lock()
        .map_err(|e| format!("锁配置失败: {}", e))?;
    *cfg = config;
    crate::config::save_config(&cfg).map_err(|e| format!("{:?}", e))?;

    // 热重载快捷键监听
    crate::hotkey::restart_hotkey_listener(app, cfg.clone());

    Ok(())
}

// ──────────────────────────────────────────────
// 后端健康检查
// ──────────────────────────────────────────────

/// 检查后端是否在线 (GET /health)。
/// 同时 emit "backend-status" 事件通知前端。
#[tauri::command]
pub async fn check_backend(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<bool, String> {
    let (server_url, token) = {
        let cfg = state
            .config
            .lock()
            .map_err(|e| format!("锁配置失败: {}", e))?;
        (
            cfg.server_url.clone(),
            cfg.token.clone().unwrap_or_default(),
        )
    };

    let url = format!("{}/health", server_url);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let alive = match client
        .get(&url)
        .header("X-VoiceInput-Token", &token)
        .send()
        .await
    {
        Ok(resp) => resp.status().is_success(),
        Err(e) => {
            log::warn!("后端健康检查失败: {}", e);
            false
        }
    };

    // emit backend-status 事件
    let _ = app.emit("backend-status", alive);

    Ok(alive)
}

// ──────────────────────────────────────────────
// 模型管理 commands
// ──────────────────────────────────────────────

/// 获取模型状态 (GET /model/status)。
/// 同时 emit "model-status" 事件通知前端。
#[tauri::command]
pub async fn get_model_status(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<ModelStatus, String> {
    let (server_url, token) = {
        let cfg = state
            .config
            .lock()
            .map_err(|e| format!("锁配置失败: {}", e))?;
        (
            cfg.server_url.clone(),
            cfg.token.clone().unwrap_or_default(),
        )
    };

    let url = format!("{}/model/status", server_url);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let resp = client
        .get(&url)
        .header("X-VoiceInput-Token", &token)
        .send()
        .await
        .map_err(|e| format!("获取模型状态失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("获取模型状态返回错误: {}", resp.status()));
    }

    let status: ModelStatus = resp
        .json()
        .await
        .map_err(|e| format!("解析模型状态失败: {}", e))?;

    // emit model-status 事件
    let _ = app.emit("model-status", &status);

    Ok(status)
}

/// 下载模型 (POST /model/download)。
#[tauri::command]
pub async fn download_model(
    state: State<'_, AppState>,
    source: Option<String>,
) -> Result<(), String> {
    let (server_url, token) = {
        let cfg = state
            .config
            .lock()
            .map_err(|e| format!("锁配置失败: {}", e))?;
        (
            cfg.server_url.clone(),
            cfg.token.clone().unwrap_or_default(),
        )
    };

    let url = format!("{}/model/download", server_url);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let mut req = client.post(&url).header("X-VoiceInput-Token", &token);
    if let Some(src) = source {
        req = req.json(&serde_json::json!({ "source": src }));
    }

    let resp = req
        .send()
        .await
        .map_err(|e| format!("下载模型请求失败: {}", e))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("下载模型返回错误: {}", body));
    }

    Ok(())
}

/// 加载模型 (POST /model/load)。
#[tauri::command]
pub async fn load_model(
    state: State<'_, AppState>,
    model_path: Option<String>,
) -> Result<(), String> {
    let (server_url, token) = {
        let cfg = state
            .config
            .lock()
            .map_err(|e| format!("锁配置失败: {}", e))?;
        (
            cfg.server_url.clone(),
            cfg.token.clone().unwrap_or_default(),
        )
    };

    let url = format!("{}/model/load", server_url);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let mut req = client.post(&url).header("X-VoiceInput-Token", &token);
    let body = if let Some(path) = model_path {
        serde_json::json!({ "model_path": path })
    } else {
        serde_json::json!({})
    };
    req = req.json(&body);

    let resp = req
        .send()
        .await
        .map_err(|e| format!("加载模型请求失败: {}", e))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("加载模型返回错误: {}", body));
    }

    Ok(())
}

/// 卸载模型 (POST /model/unload)。
#[tauri::command]
pub async fn unload_model(state: State<'_, AppState>) -> Result<(), String> {
    let (server_url, token) = {
        let cfg = state
            .config
            .lock()
            .map_err(|e| format!("锁配置失败: {}", e))?;
        (
            cfg.server_url.clone(),
            cfg.token.clone().unwrap_or_default(),
        )
    };

    let url = format!("{}/model/unload", server_url);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let resp = client
        .post(&url)
        .header("X-VoiceInput-Token", &token)
        .send()
        .await
        .map_err(|e| format!("卸载模型请求失败: {}", e))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("卸载模型返回错误: {}", body));
    }

    Ok(())
}

/// 获取模型下载进度 (GET /model/download/status)。
#[tauri::command]
pub async fn get_download_status(state: State<'_, AppState>) -> Result<DownloadStatus, String> {
    let (server_url, token) = {
        let cfg = state
            .config
            .lock()
            .map_err(|e| format!("锁配置失败: {}", e))?;
        (
            cfg.server_url.clone(),
            cfg.token.clone().unwrap_or_default(),
        )
    };

    let url = format!("{}/model/download/status", server_url);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let resp = client
        .get(&url)
        .header("X-VoiceInput-Token", &token)
        .send()
        .await
        .map_err(|e| format!("获取下载状态失败: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("获取下载状态返回错误: {}", resp.status()));
    }

    let status: DownloadStatus = resp
        .json()
        .await
        .map_err(|e| format!("解析下载状态失败: {}", e))?;

    Ok(status)
}

/// 取消模型下载 (POST /model/download/cancel)。
#[tauri::command]
pub async fn cancel_download(state: State<'_, AppState>) -> Result<(), String> {
    let (server_url, token) = {
        let cfg = state
            .config
            .lock()
            .map_err(|e| format!("锁配置失败: {}", e))?;
        (
            cfg.server_url.clone(),
            cfg.token.clone().unwrap_or_default(),
        )
    };

    let url = format!("{}/model/download/cancel", server_url);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("创建 HTTP 客户端失败: {}", e))?;

    let resp = client
        .post(&url)
        .header("X-VoiceInput-Token", &token)
        .send()
        .await
        .map_err(|e| format!("取消下载请求失败: {}", e))?;

    if !resp.status().is_success() {
        let body = resp.text().await.unwrap_or_default();
        return Err(format!("取消下载返回错误: {}", body));
    }

    log::info!("模型下载已取消");
    Ok(())
}
