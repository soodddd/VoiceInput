/**
 * VoiceInput v2 — 根组件
 *
 * 视图路由：waiting / floating / settings / download
 * 窗口大小根据视图动态调整：
 *   - floating: 280×240（紧凑悬浮窗）
 *   - settings: 500×540（设置面板）
 *   - download: 480×420（模型下载引导）
 *   - waiting:  280×160（加载中）
 */

import { useEffect, useState, useCallback, useRef } from 'react';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { getCurrentWindow, LogicalSize } from '@tauri-apps/api/window';
import { useBackend } from './hooks/useBackend';
import { useSettings } from './hooks/useSettings';
import { useRecorder } from './hooks/useRecorder';
import { loadModel, unloadModel, saveConfig } from './utils/api';
import { FloatingWindow } from './components/FloatingWindow';
import { SettingsDialog } from './components/SettingsDialog';
import { ModelDownload } from './components/ModelDownload';
import { LoadingIcon } from './components/Icons';
import type { AppView, Language } from './types';

const LANGUAGE_CYCLE: Language[] = ['auto', 'Chinese', 'English'];

interface WindowSize {
  width: number;
  height: number;
}

const VIEW_SIZES: Record<AppView, WindowSize> = {
  waiting: { width: 280, height: 160 },
  floating: { width: 280, height: 220 },
  settings: { width: 500, height: 640 },
  download: { width: 480, height: 620 },
  error: { width: 280, height: 160 },
};

const FLOATING_RESULT_HEIGHT = 320;
const FLOATING_RECORDING_HEIGHT = 240;
const FLOATING_ERROR_HEIGHT = 260;

function App(): JSX.Element {
  const {
    backendReady,
    loading,
    modelStatus,
    refreshModelStatus,
    refreshBackend,
  } = useBackend();
  const { config, updateConfig, reloadConfig } = useSettings();

  const [view, setView] = useState<AppView>('waiting');
  const [prevView, setPrevView] = useState<AppView>('floating');
  const [backendWaitExpired, setBackendWaitExpired] = useState(false);
  const [backendRetryAttempt, setBackendRetryAttempt] = useState(0);
  const sizeAppliedRef = useRef<AppView | null>(null);

  const language = (config.language as Language) || 'auto';
  const recorder = useRecorder(language, backendReady && modelStatus.loaded);
  const setRecorderError = recorder.setError;

  useEffect(() => {
    if (backendReady) {
      setBackendWaitExpired(false);
      return;
    }
    const timer = setTimeout(() => setBackendWaitExpired(true), 45_000);
    return () => clearTimeout(timer);
  }, [backendReady, backendRetryAttempt]);

  // 根据视图和录音状态动态调整窗口大小
  useEffect(() => {
    const win = getCurrentWindow();

    const applySize = async (width: number, height: number, shouldCenter: boolean): Promise<void> => {
      try {
        console.log(`[WindowSize] Setting size to ${width}x${height}, view=${view}`);
        await win.setSize(new LogicalSize(width, height));
        if (shouldCenter) {
          await win.center();
        }
        const newSize = await win.innerSize();
        console.log(`[WindowSize] New size: ${JSON.stringify(newSize)}`);
      } catch (err) {
        console.error('[WindowSize] setSize failed:', err);
      }
    };

    if (view !== 'floating') {
      if (sizeAppliedRef.current === view) return;
      const size = VIEW_SIZES[view];
      if (!size) return;

      const shouldCenter = view === 'settings' || view === 'download';
      void applySize(size.width, size.height, shouldCenter);
      sizeAppliedRef.current = view;
      return;
    }

    // floating 视图下根据状态调整高度
    let height = VIEW_SIZES.floating.height;
    if (recorder.status === 'result' && recorder.resultText) {
      height = FLOATING_RESULT_HEIGHT;
    } else if (recorder.status === 'recording' || recorder.status === 'processing') {
      height = FLOATING_RECORDING_HEIGHT;
    } else if (recorder.status === 'error') {
      height = FLOATING_ERROR_HEIGHT;
    }

    void applySize(VIEW_SIZES.floating.width, height, false);
    sizeAppliedRef.current = 'floating';
  }, [view, recorder.status, recorder.resultText]);

  // 语言切换
  const handleLanguageChange = useCallback((lang: Language) => {
    updateConfig('language', lang);
    void saveConfig({ ...config, language: lang }).catch(() => {
      setRecorderError('语言设置保存失败，请重试');
    });
  }, [config, updateConfig, setRecorderError]);

  // 根据后端/模型状态自动切换视图
  useEffect(() => {
    if (loading) {
      setView('waiting');
      return;
    }
    if (!backendReady) {
      setView(backendWaitExpired ? 'error' : 'waiting');
      return;
    }
    if (modelStatus.loaded) {
      setView((current) => (current === 'settings' ? 'settings' : 'floating'));
    } else {
      setView((current) => (current === 'settings' ? 'settings' : 'download'));
    }
  }, [backendReady, backendWaitExpired, loading, modelStatus.loaded]);

  // 全局事件监听
  useEffect(() => {
    const unlistenFns: UnlistenFn[] = [];
    let cancelled = false;
    const register = async (
      event: string,
      handler: Parameters<typeof listen>[1],
    ): Promise<void> => {
      const unlisten = await listen(event, handler);
      if (cancelled) unlisten(); else unlistenFns.push(unlisten);
    };

    const setupListeners = async (): Promise<void> => {
      await register('open-settings', () => {
          setPrevView((v) => (v === 'settings' ? 'floating' : v));
          setView('settings');
      });

      const unlistenLanguage = await listen<string>('set-language', (event) => {
          const lang = event.payload as Language;
          if (LANGUAGE_CYCLE.includes(lang)) {
            handleLanguageChange(lang);
          }
      });
      if (cancelled) unlistenLanguage(); else unlistenFns.push(unlistenLanguage);

      await register('language-cycle', () => {
          const idx = LANGUAGE_CYCLE.indexOf(language);
          const next = LANGUAGE_CYCLE[(idx + 1) % LANGUAGE_CYCLE.length];
          handleLanguageChange(next);
      });

      await register('load-model', () => {
          void loadModel().then(() => void refreshModelStatus());
      });

      await register('unload-model', () => {
          void unloadModel().then(() => void refreshModelStatus());
      });
    };

    void setupListeners();

    return () => {
      cancelled = true;
      unlistenFns.forEach((fn) => fn());
    };
  }, [language, handleLanguageChange, refreshModelStatus]);

  const handleOpenSettings = useCallback(() => {
    setPrevView(view === 'settings' ? 'floating' : view);
    setView('settings');
  }, [view]);

  const handleSettingsSave = useCallback(() => {
    void reloadConfig();
    setView(prevView);
  }, [prevView, reloadConfig]);

  const handleSettingsCancel = useCallback(() => {
    setView(prevView);
  }, [prevView]);

  const handleModelLoaded = useCallback(() => {
    void refreshModelStatus();
    setView('floating');
  }, [refreshModelStatus]);

  // 等待/初始化界面
  if (view === 'waiting') {
    return (
      <div
        className="flex h-screen w-screen flex-col items-center justify-center"
        style={{
          backgroundColor: '#FFFFFF',
          borderRadius: '14px',
          border: '1px solid rgba(0,0,0,0.08)',
        }}
      >
        <div className="mb-3">
          <LoadingIcon size={28} color="#2563EB" />
        </div>
        <p className="text-xs font-medium" style={{ color: '#64748B' }}>
          {backendReady ? '正在初始化...' : '正在启动后端服务...'}
        </p>
      </div>
    );
  }

  if (view === 'error') {
    return (
      <div
        className="flex h-screen w-screen flex-col items-center justify-center px-5 text-center"
        style={{
          backgroundColor: '#FFFFFF',
          borderRadius: '14px',
          border: '1px solid rgba(0,0,0,0.08)',
        }}
      >
        <p className="text-sm font-semibold" style={{ color: '#DC2626' }}>
          本地语音服务启动失败
        </p>
        <p className="mt-2 text-xs" style={{ color: '#64748B' }}>
          请确认完整运行包中的 asr_backend/_internal 未被移动或删除。
        </p>
        <button
          type="button"
          className="mt-3 rounded-lg px-4 py-2 text-xs font-medium text-white"
          style={{ backgroundColor: '#2563EB' }}
          onClick={() => {
            setBackendWaitExpired(false);
            setBackendRetryAttempt((attempt) => attempt + 1);
            void refreshBackend();
          }}
        >
          重新检查
        </button>
      </div>
    );
  }

  // 设置面板
  if (view === 'settings') {
    return (
      <div
        className="h-screen w-screen"
        style={{ backgroundColor: '#F5F5F7', borderRadius: '12px', overflow: 'hidden' }}
      >
        <SettingsDialog onSave={handleSettingsSave} onCancel={handleSettingsCancel} />
      </div>
    );
  }

  // 模型下载引导
  if (view === 'download') {
    return (
      <div
        className="h-screen w-screen"
        style={{ backgroundColor: '#F5F5F7', borderRadius: '12px', overflow: 'hidden' }}
      >
        <ModelDownload
          onModelLoaded={handleModelLoaded}
          existingModelPath={modelStatus.model_path}
        />
      </div>
    );
  }

  // 悬浮窗（主界面）
  return (
    <div
      className="flex h-screen w-screen items-center justify-center"
      style={{ backgroundColor: 'transparent' }}
    >
      <FloatingWindow
        status={recorder.status}
        resultText={recorder.resultText}
        volumeLevel={recorder.volumeLevel}
        recordingDuration={recorder.recordingDuration}
        errorMessage={recorder.errorMessage}
        language={language}
        onLanguageChange={handleLanguageChange}
        onClearResult={recorder.clearResult}
        onOpenSettings={handleOpenSettings}
        manualStart={recorder.manualStart}
        manualStop={recorder.manualStop}
      />
    </div>
  );
}

export default App;
