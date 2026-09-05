//! VoiceInput v2 — 麦克风录音模块
//!
//! 基于 cpal (WASAPI 后端) 打开系统输入设备，采集 16kHz 单声道 i16 PCM 数据。
//! 录音过程中每 50ms 计算一次 RMS 音量级别，通过 Tauri 事件 "audio-level" 推送给前端。
//! 停止录音时将缓冲区封装为 WAV 字节流返回。

use crate::audio_utils::{compute_rms_level, samples_to_wav};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

// cpal::Stream contains a NotSendSync marker for cross-platform compatibility
// (see cpal source comments). On Windows (WASAPI), the stream is safe to send
// because Recorder is always accessed via Arc<Mutex<Recorder>>, guaranteeing
// exclusive access. This unsafe impl is the standard workaround for cpal.
unsafe impl Send for Recorder {}
unsafe impl Sync for Recorder {}

/// 音频输入设备信息（与前端 src/types.ts 的 AudioDevice 接口对应）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDeviceInfo {
    /// 设备索引（从 0 开始）
    pub index: i32,
    /// 设备名称
    pub name: String,
    /// 声道数
    pub channels: u16,
    /// 是否为系统默认输入设备
    pub is_default: bool,
}

pub struct RecordingOptions {
    pub device_index: Option<i32>,
    pub device_name: Option<String>,
    pub target_sample_rate: u32,
    pub vad_enabled: bool,
    pub max_record_sec: u32,
    pub silence_threshold_db: i32,
}

/// 枚举系统所有可用输入设备。
///
/// 遍历 cpal 默认主机的输入设备列表，返回每个设备的索引、名称、声道数
/// 以及是否为默认设备的标记。若枚举失败则返回空 Vec。
pub fn list_input_devices() -> Vec<AudioDeviceInfo> {
    let host = cpal::default_host();

    // 获取默认输入设备名称，用于 is_default 比较
    let default_name = host
        .default_input_device()
        .and_then(|d| d.name().ok());

    let devices = match host.input_devices() {
        Ok(iter) => iter,
        Err(e) => {
            log::warn!("枚举输入设备失败: {}", e);
            return Vec::new();
        }
    };

    let mut result = Vec::new();
    for (index, device) in (0_i32..).zip(devices) {
        let name = device.name().unwrap_or_else(|_| "<unknown>".to_string());
        let channels = device
            .default_input_config()
            .map(|c| c.channels())
            .unwrap_or(0);
        let is_default = default_name
            .as_ref()
            .map(|dn| dn == &name)
            .unwrap_or(false);

        result.push(AudioDeviceInfo {
            index,
            name,
            channels,
            is_default,
        });
    }

    result
}

/// 录音器，管理一次完整的录音会话（start → stop）。
///
/// 内部状态使用 `Arc` 共享给 cpal 音频回调线程。
/// 一次只能有一个活跃的录音会话。
pub struct Recorder {
    /// 是否正在录音的标志位（原子操作，跨线程安全）
    recording: Arc<AtomicBool>,
    /// 音频采样缓冲区（i16 单声道）
    buffer: Arc<Mutex<Vec<i16>>>,
    /// 活跃的 cpal 音频流（stop 时 drop）
    stream: Option<cpal::Stream>,
    /// Dedicated thread that owns the WASAPI stream for its whole lifetime.
    worker: Option<JoinHandle<()>>,
    /// Sends the latest level to the WebView without blocking the realtime
    /// cpal callback.
    level_sampler: Option<JoinHandle<()>>,
    /// 实际使用的采样率
    sample_rate: u32,
}

fn append_bounded(
    buffer: &Arc<Mutex<Vec<i16>>>,
    samples: &[i16],
    max_samples: usize,
) -> bool {
    if let Ok(mut data) = buffer.lock() {
        let remaining = max_samples.saturating_sub(data.len());
        data.extend_from_slice(&samples[..samples.len().min(remaining)]);
        return data.len() >= max_samples;
    }
    false
}

impl Recorder {
    /// 创建新的录音器实例（未开始录音）。
    pub fn new() -> Self {
        Recorder {
            recording: Arc::new(AtomicBool::new(false)),
            buffer: Arc::new(Mutex::new(Vec::new())),
            stream: None,
            worker: None,
            level_sampler: None,
            sample_rate: 16000,
        }
    }

    /// Start capture on a dedicated thread.
    ///
    /// WASAPI streams are created and released on the same worker thread. The
    /// public Tauri command only waits for stream startup, then returns while
    /// the worker owns the native stream. This keeps COM/thread affinity out
    /// of the command lifecycle and makes desktop capture match the probe.
    pub fn start(
        &mut self,
        app: AppHandle,
        options: RecordingOptions,
    ) -> Result<(), String> {
        if self.recording.load(Ordering::SeqCst) {
            return Err("已经在录音中".to_string());
        }
        if self.worker.is_some() {
            return Err("上一次录音线程尚未退出".to_string());
        }

        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
        let mut worker_recorder = Recorder {
            recording: self.recording.clone(),
            buffer: self.buffer.clone(),
            stream: None,
            worker: None,
            level_sampler: None,
            sample_rate: self.sample_rate,
        };
        let worker = thread::spawn(move || {
            let result = worker_recorder.start_legacy(app, options);
            let sample_rate = worker_recorder.sample_rate;
            let started = result.is_ok();
            let _ = ready_tx.send((result, sample_rate));
            if started {
                while worker_recorder.is_recording() {
                    thread::sleep(Duration::from_millis(20));
                }
            }
            if let Some(sampler) = worker_recorder.level_sampler.take() {
                let _ = sampler.join();
            }
            // worker_recorder drops here, releasing the WASAPI stream on the
            // same thread that created it.
        });

        let (result, sample_rate) = match ready_rx.recv_timeout(Duration::from_secs(15)) {
            Ok(value) => value,
            Err(_) => {
                let _ = worker.join();
                return Err("启动音频流超时".to_string());
            }
        };
        if let Err(error) = result {
            let _ = worker.join();
            return Err(error);
        }
        self.sample_rate = sample_rate;
        self.worker = Some(worker);
        Ok(())
    }

    /// 开始录音。
    ///
    /// # 参数
    /// - `app`: Tauri AppHandle，用于 emit "audio-level" 事件
    /// - `device`: 可选的设备索引（None = 系统默认输入设备）
    /// - `sample_rate`: 目标采样率（通常 16000）
    /// - `vad_enabled`: 是否启用 VAD 静音自动停止
    ///
    /// # 流程
    /// 1. 通过 cpal 获取默认主机和输入设备
    /// 2. 创建 StreamConfig（单声道、i16、目标采样率）
    /// 3. 启动音频流，回调中收集 samples 到 buffer
    /// 4. 每 50ms 计算一次 RMS 并 emit "audio-level" 事件
    /// 5. 若 VAD 启用，检测持续静音 2 秒后 emit "vad-silence-detected"
    fn start_legacy(
        &mut self,
        app: AppHandle,
        options: RecordingOptions,
    ) -> Result<(), String> {
        if self.recording.load(Ordering::SeqCst) {
            return Err("已经在录音中".to_string());
        }

        // 清空缓冲区
        {
            let mut buf = self.buffer.lock().map_err(|e| format!("锁缓冲区失败: {}", e))?;
            buf.clear();
        }
        // 获取音频主机和设备
        let host = cpal::default_host();

        let input_device = if let Some(name) = options.device_name.filter(|name| !name.is_empty()) {
            host.input_devices()
                .map_err(|e| format!("枚举输入设备失败: {}", e))?
                .find(|candidate| candidate.name().ok().as_deref() == Some(name.as_str()))
                .ok_or_else(|| format!("找不到输入设备: {}", name))?
        } else if let Some(idx) = options.device_index {
            if idx < 0 {
                return Err("输入设备索引不能为负数".to_string());
            }
            // 尝试按索引选择设备
            let mut devices = host
                .input_devices()
                .map_err(|e| format!("枚举输入设备失败: {}", e))?;
            devices
                .nth(idx as usize)
                .ok_or_else(|| format!("找不到索引为 {} 的输入设备", idx))?
        } else {
            host.default_input_device()
                .ok_or_else(|| "找不到默认输入设备".to_string())?
        };

        log::info!(
            "使用输入设备: {:?}",
            input_device.name().unwrap_or_default()
        );

        // 创建流配置
        let supported_config = input_device
            .default_input_config()
            .map_err(|e| format!("获取默认输入配置失败: {}", e))?;

        // Use one complete supported configuration.  Mixing the default sample
        // format/channels with a rate taken from another range can make WASAPI
        // stream construction fail.  Python performs the final 16 kHz resample.
        let actual_sample_rate = supported_config.sample_rate().0;
        if actual_sample_rate != options.target_sample_rate {
            log::warn!(
                "设备原生采样率为 {}Hz；后端将重采样到 {}Hz",
                actual_sample_rate,
                options.target_sample_rate,
            );
        }

        let channels = supported_config.channels().min(2); // 最多 2 声道，取 min
        let sample_format = supported_config.sample_format();

        let stream_config = cpal::StreamConfig {
            channels,
            sample_rate: cpal::SampleRate(actual_sample_rate),
            buffer_size: cpal::BufferSize::Default,
        };

        log::info!(
            "录音配置: 采样率={}Hz, 声道={}, 格式={:?}",
            actual_sample_rate,
            channels,
            sample_format
        );

        // 准备共享状态
        let recording_clone = self.recording.clone();
        let buffer_clone = self.buffer.clone();
        let app_clone = app.clone();
        let channels_us = channels as usize;
        let last_emit = Arc::new(Mutex::new(Instant::now()));
        let max_samples =
            actual_sample_rate as usize * options.max_record_sec.max(1) as usize;
        let max_triggered = Arc::new(AtomicBool::new(false));
        // Keep the highest observed RMS level for actionable VAD diagnostics.
        // This makes a silent-device/route problem distinguishable from a
        // speech-recognition problem without storing or logging audio data.
        let max_rms_level = Arc::new(Mutex::new(0.0_f32));

        // P2-02: VAD 静音检测共享状态
        let record_start = Arc::new(Mutex::new(Instant::now()));
        let last_sound_time = Arc::new(Mutex::new(Instant::now()));
        let vad_triggered = Arc::new(AtomicBool::new(false));
        let current_level = Arc::new(std::sync::atomic::AtomicU32::new(0.0_f32.to_bits()));
        let vad_enabled_clone = options.vad_enabled;
        // VAD 参数
        let vad_silence_threshold =
            10_f32.powf(options.silence_threshold_db as f32 / 20.0);
        const VAD_MIN_RECORD_SEC: f64 = 1.0; // 最小录音时长（秒），避免一开始就触发
        const VAD_SILENCE_DURATION_SEC: f64 = 2.0; // 持续静音多久后触发

        // 根据采样格式构建流
        let stream = match sample_format {
            cpal::SampleFormat::I16 => {
                let last_emit_inner = last_emit.clone();
                let last_sound_inner = last_sound_time.clone();
                let record_start_inner = record_start.clone();
                let vad_triggered_inner = vad_triggered.clone();
                let app_vad = app_clone.clone();
                let max_level_inner = max_rms_level.clone();
                let current_level_inner = current_level.clone();
                input_device
                    .build_input_stream(
                        &stream_config,
                        move |data: &[i16], _: &cpal::InputCallbackInfo| {
                            if !recording_clone.load(Ordering::SeqCst) {
                                return;
                            }
                            // 多声道转单声道：取各声道平均值
                            let mono: Vec<i16> = if channels_us > 1 {
                                data.chunks(channels_us)
                                    .map(|chunk| {
                                        let sum: i64 =
                                            chunk.iter().map(|&s| s as i64).sum();
                                        (sum / chunk.len() as i64) as i16
                                    })
                                    .collect()
                            } else {
                                data.to_vec()
                            };

                            // 追加到缓冲区
                            if append_bounded(&buffer_clone, &mono, max_samples)
                                && !max_triggered.swap(true, Ordering::SeqCst)
                            {
                                let _ = app_clone.emit("recording-max-duration", ());
                            }

                            // 每 50ms emit 一次音量级别 + VAD 检测
                            if let Ok(mut last) = last_emit_inner.lock() {
                                if last.elapsed() >= Duration::from_millis(50) {
                                    let level = compute_rms_level(&mono);
                                    if let Ok(mut max_level) = max_level_inner.lock() {
                                        *max_level = (*max_level).max(level);
                                    }
                                    current_level_inner.store(level.to_bits(), Ordering::Relaxed);
                                    // Do not synchronously cross into the
                                    // WebView from the real-time audio
                                    // callback. The UI meter is updated by a
                                    // lightweight sampler thread below.
                                    *last = Instant::now();

                                    // P2-02: VAD 静音检测
                                    if vad_enabled_clone && !vad_triggered_inner.load(Ordering::SeqCst) {
                                        let elapsed_sec = record_start_inner.lock().unwrap_or_else(|e| e.into_inner()).elapsed().as_secs_f64();
                                        if elapsed_sec >= VAD_MIN_RECORD_SEC {
                                            if level > vad_silence_threshold {
                                                if let Ok(mut t) = last_sound_inner.lock() {
                                                    *t = Instant::now();
                                                }
                                            } else {
                                                let silence_sec = {
                                                    let t = last_sound_inner.lock().unwrap_or_else(|e| e.into_inner());
                                                    t.elapsed().as_secs_f64()
                                                };
                                                if silence_sec >= VAD_SILENCE_DURATION_SEC {
                                                    vad_triggered_inner.store(true, Ordering::SeqCst);
                                                    let max_level = max_level_inner
                                                        .lock()
                                                        .map(|value| *value)
                                                        .unwrap_or(0.0);
                                                    log::info!("VAD: 检测到 {:.1}s 静音，自动停止录音 (当前 RMS={:.6}, 最高 RMS={:.6})", silence_sec, level, max_level);
                                                    let _ = app_vad.emit("vad-silence-detected", ());
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                        |err| {
                            log::error!("音频流错误: {}", err);
                        },
                        None,
                    )
                    .map_err(|e| format!("构建输入流失败: {}", e))?
            }
            cpal::SampleFormat::F32 => {
                let last_emit_inner = last_emit.clone();
                let last_sound_inner = last_sound_time.clone();
                let record_start_inner = record_start.clone();
                let vad_triggered_inner = vad_triggered.clone();
                let app_vad = app_clone.clone();
                let max_level_inner = max_rms_level.clone();
                let current_level_inner = current_level.clone();
                input_device
                    .build_input_stream(
                        &stream_config,
                        move |data: &[f32], _: &cpal::InputCallbackInfo| {
                            if !recording_clone.load(Ordering::SeqCst) {
                                return;
                            }
                            // f32 → i16 转换
                            let i16_data: Vec<i16> = if channels_us > 1 {
                                data.chunks(channels_us)
                                    .map(|chunk| {
                                        let sum: f32 =
                                            chunk.iter().copied().sum::<f32>()
                                                / chunk.len() as f32;
                                        (sum.clamp(-1.0, 1.0) * 32767.0) as i16
                                    })
                                    .collect()
                            } else {
                                data.iter()
                                    .map(|&s| (s.clamp(-1.0, 1.0) * 32767.0) as i16)
                                    .collect()
                            };

                            if append_bounded(&buffer_clone, &i16_data, max_samples)
                                && !max_triggered.swap(true, Ordering::SeqCst)
                            {
                                let _ = app_clone.emit("recording-max-duration", ());
                            }

                            if let Ok(mut last) = last_emit_inner.lock() {
                                if last.elapsed() >= Duration::from_millis(50) {
                                    let level = compute_rms_level(&i16_data);
                                    if let Ok(mut max_level) = max_level_inner.lock() {
                                        *max_level = (*max_level).max(level);
                                    }
                                    current_level_inner.store(level.to_bits(), Ordering::Relaxed);
                                    // UI audio-level delivery is decoupled from
                                    // this real-time callback.
                                    *last = Instant::now();

                                    // P2-02: VAD 静音检测
                                    if vad_enabled_clone && !vad_triggered_inner.load(Ordering::SeqCst) {
                                        let elapsed_sec = record_start_inner.lock().unwrap_or_else(|e| e.into_inner()).elapsed().as_secs_f64();
                                        if elapsed_sec >= VAD_MIN_RECORD_SEC {
                                            if level > vad_silence_threshold {
                                                if let Ok(mut t) = last_sound_inner.lock() {
                                                    *t = Instant::now();
                                                }
                                            } else {
                                                let silence_sec = {
                                                    let t = last_sound_inner.lock().unwrap_or_else(|e| e.into_inner());
                                                    t.elapsed().as_secs_f64()
                                                };
                                                if silence_sec >= VAD_SILENCE_DURATION_SEC {
                                                    vad_triggered_inner.store(true, Ordering::SeqCst);
                                                    let max_level = max_level_inner
                                                        .lock()
                                                        .map(|value| *value)
                                                        .unwrap_or(0.0);
                                                    log::info!("VAD: 检测到 {:.1}s 静音，自动停止录音 (当前 RMS={:.6}, 最高 RMS={:.6})", silence_sec, level, max_level);
                                                    let _ = app_vad.emit("vad-silence-detected", ());
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                        |err| {
                            log::error!("音频流错误: {}", err);
                        },
                        None,
                    )
                    .map_err(|e| format!("构建输入流失败: {}", e))?
            }
            cpal::SampleFormat::U16 => {
                let last_emit_inner = last_emit.clone();
                let last_sound_inner = last_sound_time.clone();
                let record_start_inner = record_start.clone();
                let vad_triggered_inner = vad_triggered.clone();
                let app_vad = app_clone.clone();
                let max_level_inner = max_rms_level.clone();
                let current_level_inner = current_level.clone();
                input_device
                    .build_input_stream(
                        &stream_config,
                        move |data: &[u16], _: &cpal::InputCallbackInfo| {
                            if !recording_clone.load(Ordering::SeqCst) {
                                return;
                            }
                            let i16_data: Vec<i16> = if channels_us > 1 {
                                data.chunks(channels_us)
                                    .map(|chunk| {
                                        let sum: i64 = chunk
                                            .iter()
                                            .map(|&s| (s as i32 - 32768) as i64)
                                            .sum();
                                        (sum / chunk.len() as i64) as i16
                                    })
                                    .collect()
                            } else {
                                data.iter()
                                    .map(|&s| (s as i32 - 32768) as i16)
                                    .collect()
                            };

                            if append_bounded(&buffer_clone, &i16_data, max_samples)
                                && !max_triggered.swap(true, Ordering::SeqCst)
                            {
                                let _ = app_clone.emit("recording-max-duration", ());
                            }

                            if let Ok(mut last) = last_emit_inner.lock() {
                                if last.elapsed() >= Duration::from_millis(50) {
                                    let level = compute_rms_level(&i16_data);
                                    if let Ok(mut max_level) = max_level_inner.lock() {
                                        *max_level = (*max_level).max(level);
                                    }
                                    current_level_inner.store(level.to_bits(), Ordering::Relaxed);
                                    // UI audio-level delivery is decoupled from
                                    // this real-time callback.
                                    *last = Instant::now();

                                    // P2-02: VAD 静音检测
                                    if vad_enabled_clone && !vad_triggered_inner.load(Ordering::SeqCst) {
                                        let elapsed_sec = record_start_inner.lock().unwrap_or_else(|e| e.into_inner()).elapsed().as_secs_f64();
                                        if elapsed_sec >= VAD_MIN_RECORD_SEC {
                                            if level > vad_silence_threshold {
                                                if let Ok(mut t) = last_sound_inner.lock() {
                                                    *t = Instant::now();
                                                }
                                            } else {
                                                let silence_sec = {
                                                    let t = last_sound_inner.lock().unwrap_or_else(|e| e.into_inner());
                                                    t.elapsed().as_secs_f64()
                                                };
                                                if silence_sec >= VAD_SILENCE_DURATION_SEC {
                                                    vad_triggered_inner.store(true, Ordering::SeqCst);
                                                    let max_level = max_level_inner
                                                        .lock()
                                                        .map(|value| *value)
                                                        .unwrap_or(0.0);
                                                    log::info!("VAD: 检测到 {:.1}s 静音，自动停止录音 (当前 RMS={:.6}, 最高 RMS={:.6})", silence_sec, level, max_level);
                                                    let _ = app_vad.emit("vad-silence-detected", ());
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        },
                        |err| {
                            log::error!("音频流错误: {}", err);
                        },
                        None,
                    )
                    .map_err(|e| format!("构建输入流失败: {}", e))?
            }
            other => {
                return Err(format!("不支持的采样格式: {:?}", other));
            }
        };

        stream.play().map_err(|e| format!("启动音频流失败: {}", e))?;
        // Device initialization can take seconds. Start VAD timing only when
        // the stream is ready, before allowing callbacks to accept samples.
        let started = Instant::now();
        *record_start.lock().unwrap_or_else(|e| e.into_inner()) = started;
        *last_sound_time.lock().unwrap_or_else(|e| e.into_inner()) = started;
        self.sample_rate = actual_sample_rate;
        self.recording.store(true, Ordering::SeqCst);
        self.stream = Some(stream);

        // Keep UI updates off the audio callback.  The callback only stores
        // an atomic snapshot; this sampler is allowed to cross into Tauri's
        // WebView event system and is joined during stop.
        let meter_recording = self.recording.clone();
        let meter_level = current_level.clone();
        self.level_sampler = Some(thread::spawn(move || {
            while meter_recording.load(Ordering::SeqCst) {
                let level = f32::from_bits(meter_level.load(Ordering::Relaxed));
                let _ = app.emit("audio-level", level);
                thread::sleep(Duration::from_millis(50));
            }
        }));

        log::info!("录音已开始 ({}Hz)", actual_sample_rate);
        Ok(())
    }

    /// 停止录音并返回 WAV 字节流。
    ///
    /// 1. 设置 recording 标志为 false
    /// 2. drop 音频流
    /// 3. 从缓冲区取出所有采样数据
    /// 4. 封装为 WAV 格式返回
    pub fn stop(&mut self) -> Result<Vec<u8>, String> {
        if !self.recording.load(Ordering::SeqCst) {
            return Err("当前未在录音".to_string());
        }

        self.recording.store(false, Ordering::SeqCst);

        // Ask the dedicated owner thread to exit, then wait until it has
        // dropped the native WASAPI stream before reading the buffer.
        if let Some(worker) = self.worker.take() {
            worker
                .join()
                .map_err(|_| "录音线程异常退出".to_string())?;
        }

        // 停止并释放音频流
        if let Some(stream) = self.stream.take() {
            // drop stream 会停止录音
            drop(stream);
        }

        // 取出缓冲区数据
        let samples = {
            let mut buf = self
                .buffer
                .lock()
                .map_err(|e| format!("锁缓冲区失败: {}", e))?;
            let data = buf.clone();
            buf.clear();
            data
        };

        let captured_rms = compute_rms_level(&samples);
        let captured_peak = samples
            .iter()
            .map(|sample| (*sample as f32 / 32768.0).abs())
            .fold(0.0_f32, f32::max);

        if samples.is_empty() {
            return Err("录音数据为空".to_string());
        }

        log::info!(
            "录音结束，共 {} 个采样 ({:.1}秒 @ {}Hz, RMS={:.6}, 峰值={:.6})",
            samples.len(),
            samples.len() as f32 / self.sample_rate as f32,
            self.sample_rate,
            captured_rms,
            captured_peak
        );

        let wav = samples_to_wav(&samples, self.sample_rate);
        Ok(wav)
    }

    /// 返回是否正在录音。
    pub fn is_recording(&self) -> bool {
        self.recording.load(Ordering::SeqCst)
    }
}

impl Default for Recorder {
    fn default() -> Self {
        Self::new()
    }
}
