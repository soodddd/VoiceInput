/**
 * VoiceInput v2 — 配置管理 Hook
 *
 * 加载配置 (get_config)、保存配置 (save_config)，
 * 提供更新单个字段和批量保存的方法。
 */

import { useEffect, useState, useCallback, useRef } from 'react';
import { getConfig, saveConfig } from '../utils/api';
import type { AppConfig } from '../types';

/** 默认配置（与 resources/default_config.json 一致） */
const DEFAULT_CONFIG: AppConfig = {
  hotkey: 'alt+v',
  language_hotkey: 'alt+l',
  language: 'auto',
  sample_rate: 16000,
  channels: 1,
  paste_delay_ms: 800,
  clipboard_restore: true,
  input_device: null,
  normalize_audio: true,
  trim_silence: true,
  silence_threshold_db: -40,
  max_record_sec: 120,
  request_timeout_sec: 120,
  server_url: 'http://127.0.0.1:8765',
  model_path: null,
  model_strategy: 'balanced',
  auto_start: false,
  punctuation_mode: 'simple',
  auto_space_zh_en: true,
  vad_enabled: true,
  token: '',
  custom_terms: {},
};

/** useSettings Hook 返回值 */
interface UseSettingsReturn {
  /** 当前配置 */
  config: AppConfig;
  /** 是否正在加载 */
  loading: boolean;
  /** 是否正在保存 */
  saving: boolean;
  /** 配置是否已修改（与服务器端不同） */
  dirty: boolean;
  /** 更新单个配置字段 */
  updateConfig: <K extends keyof AppConfig>(key: K, value: AppConfig[K]) => void;
  /** 批量更新多个配置字段 */
  updateConfigBatch: (partial: Partial<AppConfig>) => void;
  /** 保存配置到后端 */
  saveSettings: () => Promise<boolean>;
  /** 重新加载配置 */
  reloadConfig: () => Promise<void>;
}

/**
 * 配置管理 Hook
 *
 * 组件挂载时自动加载配置，提供安全的更新和保存方法。
 * 保存时调用 Rust 层 save_config 命令持久化到 config.json。
 */
export function useSettings(): UseSettingsReturn {
  const [config, setConfig] = useState<AppConfig>(DEFAULT_CONFIG);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [dirty, setDirty] = useState(false);
  const mountedRef = useRef(true);
  const lastSavedConfigRef = useRef<AppConfig>(DEFAULT_CONFIG);

  /** 加载配置 */
  const loadConfig = useCallback(async () => {
    try {
      const cfg = await getConfig();
      if (mountedRef.current) {
        setConfig(cfg);
        lastSavedConfigRef.current = cfg;
        setDirty(false);
      }
    } catch {
      // 加载失败时使用默认配置
      if (mountedRef.current) {
        setConfig(DEFAULT_CONFIG);
        lastSavedConfigRef.current = DEFAULT_CONFIG;
      }
    } finally {
      if (mountedRef.current) {
        setLoading(false);
      }
    }
  }, []);

  useEffect(() => {
    mountedRef.current = true;
    loadConfig();

    return () => {
      mountedRef.current = false;
    };
  }, [loadConfig]);

  /** 更新单个配置字段 */
  const updateConfig = useCallback(
    <K extends keyof AppConfig>(key: K, value: AppConfig[K]): void => {
      setConfig((prev) => {
        const next = { ...prev, [key]: value };
        return next;
      });
      setDirty(true);
    },
    []
  );

  /** 批量更新多个配置字段 */
  const updateConfigBatch = useCallback((partial: Partial<AppConfig>): void => {
    setConfig((prev) => {
      const next = { ...prev, ...partial };
      return next;
    });
    setDirty(true);
  }, []);

  /** 保存配置到后端 */
  const saveSettings = useCallback(async (): Promise<boolean> => {
    setSaving(true);
    try {
      await saveConfig(config);
      if (mountedRef.current) {
        lastSavedConfigRef.current = config;
        setDirty(false);
      }
      return true;
    } catch {
      return false;
    } finally {
      if (mountedRef.current) {
        setSaving(false);
      }
    }
  }, [config]);

  /** 重新加载配置 */
  const reloadConfig = useCallback(async (): Promise<void> => {
    await loadConfig();
  }, [loadConfig]);

  return {
    config,
    loading,
    saving,
    dirty,
    updateConfig,
    updateConfigBatch,
    saveSettings,
    reloadConfig,
  };
}

export default useSettings;
