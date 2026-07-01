/**
 * VoiceInput v2 — TypeScript 类型定义
 *
 * 定义前端所有共享类型，与 Rust 层 (T02) 的数据结构保持一致。
 */

/** 应用状态枚举（悬浮窗状态机） */
export type AppStatus =
  | 'idle'          // 空闲
  | 'recording'     // 录音中
  | 'processing'    // 识别中
  | 'result'        // 结果展示
  | 'error';        // 错误

/** 语言选项 */
export type Language = 'auto' | 'Chinese' | 'English';

/** 应用视图类型（App.tsx 窗口路由） */
export type AppView = 'floating' | 'settings' | 'download' | 'waiting' | 'error';

/** 用户配置（与 Rust 层 AppConfig 结构一致） */
export interface AppConfig {
  /** 录音快捷键，如 "alt+v" */
  hotkey: string;
  /** 语言切换快捷键，如 "alt+l" */
  language_hotkey: string;
  /** 识别语言："auto" | "Chinese" | "English" */
  language: string;
  /** 采样率，默认 16000 */
  sample_rate: number;
  /** 声道数，默认 1 */
  channels: number;
  /** 粘贴延迟（毫秒），默认 800 */
  paste_delay_ms: number;
  /** 是否粘贴后恢复原剪贴板内容 */
  clipboard_restore: boolean;
  /** 输入设备索引，null 表示系统默认 */
  input_device: number | null;
  /** 是否启用音量归一化 */
  normalize_audio: boolean;
  /** 是否裁剪静音段 */
  trim_silence: boolean;
  /** 静音阈值（dB），默认 -40 */
  silence_threshold_db: number;
  /** 最大录音时长（秒），默认 120 */
  max_record_sec: number;
  /** 请求超时（秒），默认 120 */
  request_timeout_sec: number;
  /** ASR 后端地址，如 "http://127.0.0.1:8765" */
  server_url: string;
  /** 模型路径，null 表示使用默认路径 */
  model_path: string | null;
  /** 模型策略："fast" | "balanced" | "accurate" | "memory" */
  model_strategy: string;
  /** P2-05: 开机自启 */
  auto_start: boolean;
  /** P2-06: 标点模式: "raw" | "simple" | "input_method" */
  punctuation_mode: string;
  /** P2-07: 中英混排自动加空格 */
  auto_space_zh_en: boolean;
  /** P2-02: VAD 语音活动检测 */
  vad_enabled: boolean;
  /** 本地安全 token */
  token: string;
  /** 用户自定义术语词典（ASR误识别 → 正确文本） */
  custom_terms: Record<string, string>;
}

/** 模型状态（get_model_status 返回） */
export interface ModelStatus {
  /** 模型是否已加载到显存 */
  loaded: boolean;
  /** 是否正在下载 */
  downloading: boolean;
  /** 下载进度（0-100） */
  download_progress: number;
  /** 模型名称 */
  model_name: string;
  /** 推理设备，如 "cuda:0" */
  device: string;
}

/** 下载状态（get_download_status 返回） */
export interface DownloadStatus {
  /** 是否正在下载 */
  downloading: boolean;
  /** 下载进度（0-100） */
  progress: number;
  /** 下载速度（bytes/s） */
  speed: number;
  /** 错误信息，null 表示无错误 */
  error: string | null;
}

/** 音频输入设备 */
export interface AudioDevice {
  /** 设备索引 */
  index: number;
  /** 设备名称 */
  name: string;
  /** 声道数 */
  channels: number;
  /** 是否为系统默认设备 */
  is_default: boolean;
}

/** 悬浮窗状态集合 */
export interface FloatingWindowState {
  status: AppStatus;
  resultText: string;
  volumeLevel: number;
  language: Language;
  recordingDuration: number;
  errorMessage: string;
}
