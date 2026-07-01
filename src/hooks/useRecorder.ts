/**
 * VoiceInput v2 — 录音状态管理 Hook
 *
 * 管理 recording/processing 状态，监听 Tauri 事件，
 * 提供录音控制方法。
 *
 * 状态机：idle → recording → processing → result → idle
 *
 * 支持两种触发方式：
 * 1. 快捷键触发：监听 Rust 层发出的 recording-start/recording-stop 事件
 * 2. 手动触发：通过暴露的 manualStart/manualStop 方法（用于 UI 麦克风按钮）
 */

import { useEffect, useState, useRef, useCallback } from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import {
  startRecording as invokeStartRecording,
  stopRecording as invokeStopRecording,
  transcribeAndPaste as invokeTranscribeAndPaste,
} from '../utils/api';
import type { AppStatus, Language } from '../types';

/** useRecorder Hook 返回值 */
interface UseRecorderReturn {
  status: AppStatus;
  resultText: string;
  volumeLevel: number;
  recordingDuration: number;
  errorMessage: string;
  clearResult: () => void;
  setError: (message: string) => void;
  manualStart: () => Promise<void>;
  manualStop: () => Promise<void>;
}

/**
 * 录音状态管理 Hook
 *
 * @param language 当前识别语言
 */
export function useRecorder(language: Language): UseRecorderReturn {
  const [status, setStatus] = useState<AppStatus>('idle');
  const [resultText, setResultText] = useState('');
  const [volumeLevel, setVolumeLevel] = useState(0);
  const [recordingDuration, setRecordingDuration] = useState(0);
  const [errorMessage, setErrorMessage] = useState('');

  const languageRef = useRef(language);
  const statusRef = useRef<AppStatus>('idle');
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const decayTimerRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const mountedRef = useRef(true);

  /**
   * 将后端/系统错误信息转换为用户友好的中文提示。
   * 区分常见错误场景：麦克风问题、后端未就绪、网络错误、GPU 问题等。
   */
  const friendlyErrorMessage = useCallback((err: unknown, context: 'start' | 'stop'): string => {
    const raw = err instanceof Error ? err.message : String(err);
    const lower = raw.toLowerCase();

    // 麦克风相关错误
    if (lower.includes('device') && (lower.includes('not found') || lower.includes('unavailable'))) {
      return '找不到麦克风，请检查麦克风是否已连接';
    }
    if (lower.includes('permission') || lower.includes('denied') || lower.includes('access')) {
      return '麦克风访问被拒绝，请在系统设置中允许使用麦克风';
    }
    if (lower.includes('busy') || lower.includes('occupied') || lower.includes('in use')) {
      return '麦克风被其他程序占用，请关闭其他录音软件后重试';
    }
    if (lower.includes('no device') || lower.includes('nodevice')) {
      return '没有可用的麦克风，请连接麦克风后重试';
    }

    // 后端未就绪
    if (lower.includes('connection refused') || lower.includes('econnrefused')) {
      return context === 'start'
        ? '语音服务未启动，请稍候几秒再试'
        : '语音服务连接失败，请检查服务是否正常运行';
    }
    if (lower.includes('timeout') || lower.includes('timed out')) {
      return '请求超时，可能 GPU 正在加载模型，请稍候再试';
    }

    // GPU 相关
    if (lower.includes('cuda') || lower.includes('gpu') || lower.includes('out of memory')) {
      return 'GPU 显存不足或不可用，请关闭其他占显存的程序后重试';
    }

    // 网络错误（模型下载等）
    if (lower.includes('network') || lower.includes('dns') || lower.includes('connect')) {
      return '网络连接失败，请检查网络后重试';
    }

    // 空音频
    if (lower.includes('empty') || lower.includes('no audio')) {
      return '录音太短，请长按说话';
    }

    // 默认：保留原始信息但加上友好前缀
    return context === 'start'
      ? `录音启动失败：${raw}`
      : `识别失败：${raw}`;
  }, []);

  useEffect(() => {
    languageRef.current = language;
  }, [language]);

  useEffect(() => {
    statusRef.current = status;
  }, [status]);

  const startTimer = useCallback(() => {
    setRecordingDuration(0);
    timerRef.current = setInterval(() => {
      if (mountedRef.current) {
        setRecordingDuration((prev) => prev + 1);
      }
    }, 1000);
  }, []);

  const stopTimer = useCallback(() => {
    if (timerRef.current !== null) {
      clearInterval(timerRef.current);
      timerRef.current = null;
    }
  }, []);

  const startDecay = useCallback(() => {
    if (decayTimerRef.current !== null) {
      clearInterval(decayTimerRef.current);
    }
    decayTimerRef.current = setInterval(() => {
      if (mountedRef.current) {
        setVolumeLevel((prev) => {
          if (prev <= 0.01) {
            if (decayTimerRef.current !== null) {
              clearInterval(decayTimerRef.current);
              decayTimerRef.current = null;
            }
            return 0;
          }
          return prev * 0.8;
        });
      }
    }, 50);
  }, []);

  /** 内部：执行录音开始逻辑（供事件和手动调用共用） */
  const doStartRecording = useCallback(async () => {
    if (statusRef.current !== 'idle' && statusRef.current !== 'result' && statusRef.current !== 'error') {
      return;
    }
    try {
      await invokeStartRecording();
      if (mountedRef.current) {
        setStatus('recording');
        setErrorMessage('');
        setResultText('');
        startTimer();
      }
    } catch (err) {
      if (mountedRef.current) {
        setStatus('error');
        setErrorMessage(friendlyErrorMessage(err, 'start'));
      }
    }
  }, [startTimer, friendlyErrorMessage]);

  /** 内部：执行录音停止+识别+粘贴逻辑（供事件和手动调用共用） */
  const doStopRecording = useCallback(async () => {
    if (statusRef.current !== 'recording') {
      return;
    }
    stopTimer();
    if (mountedRef.current) {
      setStatus('processing');
      startDecay();
    }

    try {
      const wav = await invokeStopRecording();

      if (!wav || wav.length === 0) {
        if (mountedRef.current) {
          setStatus('idle');
        }
        return;
      }

      const text = await invokeTranscribeAndPaste(wav, languageRef.current);

      if (mountedRef.current) {
        const trimmed = (text || '').trim();
        if (!trimmed) {
          setResultText('');
          setStatus('error');
          setErrorMessage('未检测到语音，请重试');
        } else {
          setResultText(trimmed);
          setStatus('result');
        }
      }
    } catch (err) {
      if (mountedRef.current) {
        setStatus('error');
        setErrorMessage(friendlyErrorMessage(err, 'stop'));
      }
    }
  }, [stopTimer, startDecay, friendlyErrorMessage]);

  const handleAudioLevel = useCallback((level: number) => {
    if (mountedRef.current) {
      setVolumeLevel(Math.max(0, Math.min(1, level)));
    }
  }, []);

  const handleTranscribeResult = useCallback((text: string) => {
    if (mountedRef.current) {
      const trimmed = (text || '').trim();
      if (!trimmed) {
        setResultText('');
        setStatus('error');
        setErrorMessage('未检测到语音，请重试');
      } else {
        setResultText(trimmed);
        setStatus('result');
      }
    }
  }, []);

  const clearResult = useCallback(() => {
    if (mountedRef.current) {
      setResultText('');
      setErrorMessage('');
      setStatus('idle');
    }
  }, []);

  const setError = useCallback((message: string) => {
    if (mountedRef.current) {
      setErrorMessage(message);
      setStatus('error');
    }
  }, []);

  /** 手动开始录音（UI 按钮调用） */
  const manualStart = useCallback(async () => {
    await doStartRecording();
  }, [doStartRecording]);

  /** 手动停止录音并触发识别（UI 按钮调用） */
  const manualStop = useCallback(async () => {
    await doStopRecording();
  }, [doStopRecording]);

  // 监听 Tauri 事件
  useEffect(() => {
    mountedRef.current = true;
    const unlistenFns: UnlistenFn[] = [];

    const setupListeners = async (): Promise<void> => {
      unlistenFns.push(
        await listen('recording-start', () => {
          void doStartRecording();
        })
      );

      unlistenFns.push(
        await listen('recording-stop', () => {
          void doStopRecording();
        })
      );

      // P2-02: VAD 静音自动停止
      unlistenFns.push(
        await listen('vad-silence-detected', () => {
          if (statusRef.current === 'recording') {
            void doStopRecording();
          }
        })
      );

      unlistenFns.push(
        await listen<number>('audio-level', (event) => {
          handleAudioLevel(event.payload);
        })
      );

      unlistenFns.push(
        await listen<string>('transcribe-result', (event) => {
          handleTranscribeResult(event.payload);
        })
      );
    };

    void setupListeners();

    return () => {
      mountedRef.current = false;
      unlistenFns.forEach((fn) => fn());
      if (timerRef.current !== null) {
        clearInterval(timerRef.current);
      }
      if (decayTimerRef.current !== null) {
        clearInterval(decayTimerRef.current);
      }
    };
  }, [doStartRecording, doStopRecording, handleAudioLevel, handleTranscribeResult]);

  return {
    status,
    resultText,
    volumeLevel,
    recordingDuration,
    errorMessage,
    clearResult,
    setError,
    manualStart,
    manualStop,
  };
}

export default useRecorder;
