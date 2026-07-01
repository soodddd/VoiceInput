/**
 * VoiceInput v2 — Tauri invoke 封装
 *
 * 封装所有 Tauri command 调用，提供类型安全的 API 接口。
 * 命令名和参数名与 T02 Rust 层定义完全一致。
 */

import { invoke } from '@tauri-apps/api/core';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import type { AppConfig, ModelStatus, DownloadStatus, AudioDevice } from '../types';

/**
 * 开始录音
 * 调用 Rust 层 start_recording 命令，启动 cpal 麦克风输入流。
 */
export async function startRecording(): Promise<void> {
  await invoke('start_recording');
}

/**
 * 停止录音并获取 WAV 数据
 * 调用 Rust 层 stop_recording 命令，返回 PCM 16-bit WAV 字节数组。
 * @returns WAV 格式的音频字节数组
 */
export async function stopRecording(): Promise<number[]> {
  const wav = await invoke<number[]>('stop_recording');
  return wav;
}

/**
 * 语音识别并粘贴到光标位置
 * 调用 Rust 层 transcribe_and_paste 命令，发送 WAV 到 Python 后端识别，
 * 并自动将结果粘贴到当前光标位置。
 *
 * Rust 层会自动合并 config.custom_terms 与传入的 customTerms（如有），
 * 因此大多数情况下前端无需显式传递 customTerms。
 *
 * @param wav WAV 格式的音频字节数组
 * @param language 识别语言："auto" | "Chinese" | "English"
 * @param customTerms 可选的额外自定义术语（覆盖/补充配置中的术语）
 * @returns 识别出的文本
 */
export async function transcribeAndPaste(
  wav: number[],
  language: string,
  customTerms?: Record<string, string>
): Promise<string> {
  const payload: Record<string, unknown> = { wav, language };
  if (customTerms && Object.keys(customTerms).length > 0) {
    payload.custom_terms = customTerms;
  }
  const text = await invoke<string>('transcribe_and_paste', payload);
  return text;
}

/**
 * 粘贴文本到光标位置（重新插入已识别的文本）
 * 调用 Rust 层 paste_text 命令，将指定文本写入剪贴板并模拟 Ctrl+V 粘贴。
 * @param text 要粘贴的文本
 */
export async function pasteText(text: string): Promise<void> {
  await invoke('paste_text', { text });
}

/**
 * 复制文本到剪贴板
 * 使用 Tauri clipboard-manager 插件写入剪贴板。
 * @param text 要复制的文本
 */
export async function copyToClipboard(text: string): Promise<void> {
  await writeText(text);
}

/**
 * 获取用户配置
 * 调用 Rust 层 get_config 命令，返回当前配置。
 * @returns 用户配置对象
 */
export async function getConfig(): Promise<AppConfig> {
  const config = await invoke<AppConfig>('get_config');
  return config;
}

/**
 * 保存用户配置
 * 调用 Rust 层 save_config 命令，持久化配置到 config.json。
 * @param config 完整的配置对象
 */
export async function saveConfig(config: AppConfig): Promise<void> {
  await invoke('save_config', { config });
}

/**
 * 检查后端是否就绪
 * 调用 Rust 层 check_backend 命令，检查 Python sidecar 是否启动并健康。
 * @returns 后端是否就绪
 */
export async function checkBackend(): Promise<boolean> {
  const ready = await invoke<boolean>('check_backend');
  return ready;
}

/**
 * 获取模型状态
 * 调用 Rust 层 get_model_status 命令，查询模型加载/下载状态。
 * @returns 模型状态对象
 */
export async function getModelStatus(): Promise<ModelStatus> {
  const status = await invoke<ModelStatus>('get_model_status');
  return status;
}

/**
 * 开始下载模型
 * 调用 Rust 层 download_model 命令，触发模型下载。
 * @param source 下载源："modelscope" | "huggingface" | "local"
 */
export async function downloadModel(source: string): Promise<void> {
  await invoke('download_model', { source });
}

/**
 * 加载模型到显存
 * 调用 Rust 层 load_model 命令，将模型加载到 GPU。
 */
export async function loadModel(): Promise<void> {
  await invoke('load_model');
}

/**
 * 卸载模型释放显存
 * 调用 Rust 层 unload_model 命令，释放 GPU 显存。
 */
export async function unloadModel(): Promise<void> {
  await invoke('unload_model');
}

/**
 * 获取下载进度状态
 * 调用 Rust 层 get_download_status 命令，查询当前下载进度。
 * @returns 下载状态对象
 */
export async function getDownloadStatus(): Promise<DownloadStatus> {
  const status = await invoke<DownloadStatus>('get_download_status');
  return status;
}

/**
 * 取消模型下载
 * 调用 Rust 层 cancel_download 命令，向 Python 后端发送取消请求。
 * 后端设置取消标志，下载 worker 会在下一次检查时终止。
 */
export async function cancelDownload(): Promise<void> {
  await invoke('cancel_download');
}

/**
 * 获取音频输入设备列表
 * 调用 Rust 层 get_devices 命令，枚举系统可用麦克风。
 * 如果 Rust 层未实现此命令，会抛出异常。
 * @returns 设备列表
 */
export async function getDevices(): Promise<AudioDevice[]> {
  const devices = await invoke<AudioDevice[]>('get_devices');
  return devices;
}

/**
 * 隐藏当前窗口到系统托盘
 * 使用 Tauri 窗口 API 隐藏窗口，不退出程序。
 */
export async function hideWindow(): Promise<void> {
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  const appWindow = getCurrentWindow();
  await appWindow.hide();
}

/**
 * 开始拖拽窗口
 * 使用 Tauri 窗口 API 启动原生窗口拖拽。
 */
export async function startDragging(): Promise<void> {
  const { getCurrentWindow } = await import('@tauri-apps/api/window');
  const appWindow = getCurrentWindow();
  await appWindow.startDragging();
}
