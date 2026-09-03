/**
 * VoiceInput v2 — 后端状态轮询 Hook
 *
 * 轮询 check_backend（每 3s）和 get_model_status（每 5s），
 * 返回后端就绪状态和模型状态。
 */

import { useEffect, useState, useRef, useCallback } from 'react';
import { checkBackend, getModelStatus } from '../utils/api';
import type { ModelStatus } from '../types';

/** 默认模型状态（未加载） */
const DEFAULT_MODEL_STATUS: ModelStatus = {
  loaded: false,
  installed: false,
  model_path: null,
  downloading: false,
  download_progress: 0,
  download_state: 'idle',
  download_message: '',
  download_error: null,
  strategy: null,
};

/** useBackend Hook 返回值 */
interface UseBackendReturn {
  /** 后端是否就绪 */
  backendReady: boolean;
  /** 是否正在加载（首次检查中） */
  loading: boolean;
  /** 模型状态 */
  modelStatus: ModelStatus;
  /** 手动刷新模型状态 */
  refreshModelStatus: () => Promise<void>;
  /** 手动重试后端健康检查 */
  refreshBackend: () => Promise<void>;
}

/** 后端检查间隔（毫秒） */
const BACKEND_CHECK_INTERVAL = 3000;
/** 模型状态检查间隔（毫秒） */
const MODEL_CHECK_INTERVAL = 5000;

/**
 * 后端状态轮询 Hook
 *
 * 组件挂载后立即检查后端和模型状态，然后按设定间隔持续轮询。
 * 后端未就绪时缩短检查间隔以加快响应。
 */
export function useBackend(): UseBackendReturn {
  const [backendReady, setBackendReady] = useState(false);
  const [loading, setLoading] = useState(true);
  const [modelStatus, setModelStatus] = useState<ModelStatus>(DEFAULT_MODEL_STATUS);
  const mountedRef = useRef(true);
  const backendInFlightRef = useRef(false);
  const modelInFlightRef = useRef(false);

  /** 检查后端状态 */
  const checkBackendStatus = useCallback(async () => {
    if (backendInFlightRef.current) return;
    backendInFlightRef.current = true;
    try {
      const ready = await checkBackend();
      if (mountedRef.current) {
        setBackendReady(ready);
        setLoading(false);
      }
    } catch {
      // 后端检查失败，保持 false
      if (mountedRef.current) {
        setBackendReady(false);
        setLoading(false);
      }
    } finally {
      backendInFlightRef.current = false;
    }
  }, []);

  /** 检查模型状态 */
  const checkModelStatus = useCallback(async () => {
    if (modelInFlightRef.current) return;
    modelInFlightRef.current = true;
    try {
      const status = await getModelStatus();
      if (mountedRef.current) {
        setModelStatus(status);
      }
    } catch {
      // 模型状态查询失败，保持默认值
    } finally {
      modelInFlightRef.current = false;
    }
  }, []);

  /** 手动刷新模型状态 */
  const refreshModelStatus = useCallback(async () => {
    await checkModelStatus();
  }, [checkModelStatus]);

  useEffect(() => {
    mountedRef.current = true;

    // 立即执行首次检查
    checkBackendStatus();
    checkModelStatus();

    // 设置定时轮询
    const backendTimer = setInterval(() => {
      checkBackendStatus();
    }, BACKEND_CHECK_INTERVAL);

    const modelTimer = setInterval(() => {
      checkModelStatus();
    }, MODEL_CHECK_INTERVAL);

    return () => {
      mountedRef.current = false;
      clearInterval(backendTimer);
      clearInterval(modelTimer);
    };
  }, [checkBackendStatus, checkModelStatus]);

  return {
    backendReady,
    loading,
    modelStatus,
    refreshModelStatus,
    refreshBackend: checkBackendStatus,
  };
}

export default useBackend;
