/**
 * VoiceInput v2 — 模型下载引导界面
 *
 * 480×600px，居中显示。包含：
 * - 欢迎标题 + 隐私说明
 * - 下载源选择：ModelScope（推荐）/ HuggingFace / 本地路径
 * - 下载进度条（轮询 get_download_status 每 1s）
 * - 下载完成后显示"加载模型"按钮 → 调 invoke('load_model')
 */

import { useEffect, useState, useCallback, useRef } from 'react';
import {
  downloadModel,
  loadModel,
  getDownloadStatus,
  cancelDownload,
} from '../utils/api';
import { Toast } from './Toast';
import { DownloadIcon, ShieldIcon, LoadingIcon } from './Icons';
import type { DownloadStatus } from '../types';

/** ModelDownload 组件 Props */
interface ModelDownloadProps {
  /** 模型加载完成回调 */
  onModelLoaded: () => void;
}

/** 下载源选项 */
interface DownloadSource {
  key: string;
  label: string;
  description: string;
  recommended?: boolean;
}

const DOWNLOAD_SOURCES: DownloadSource[] = [
  {
    key: 'modelscope',
    label: 'ModelScope',
    description: '推荐国内用户，下载速度快',
    recommended: true,
  },
  {
    key: 'huggingface',
    label: 'HuggingFace',
    description: '国际用户使用',
  },
  {
    key: 'local',
    label: '本地路径',
    description: '已有模型文件，跳过下载',
  },
];

/** 下载状态枚举 */
type DownloadPhase = 'idle' | 'downloading' | 'completed' | 'failed' | 'loading';

/**
 * 格式化下载速度
 * @param speed bytes/s
 * @returns 格式化字符串如 "1.5 MB/s"
 */
function formatSpeed(speed: number): string {
  if (speed <= 0) return '--';
  if (speed < 1024) return `${Math.round(speed)} B/s`;
  if (speed < 1024 * 1024) return `${(speed / 1024).toFixed(1)} KB/s`;
  return `${(speed / (1024 * 1024)).toFixed(1)} MB/s`;
}

/**
 * 模型下载引导界面
 */
export function ModelDownload({ onModelLoaded }: ModelDownloadProps): JSX.Element {
  const [selectedSource, setSelectedSource] = useState<string>('modelscope');
  const [phase, setPhase] = useState<DownloadPhase>('idle');
  const [progress, setProgress] = useState(0);
  const [speed, setSpeed] = useState(0);
  const [errorMsg, setErrorMsg] = useState('');
  const [toastMessage, setToastMessage] = useState('');
  const mountedRef = useRef(true);

  /** 轮询下载状态 */
  const pollDownloadStatus = useCallback(async () => {
    try {
      const status: DownloadStatus = await getDownloadStatus();
      if (!mountedRef.current) return;

      if (status.downloading) {
        setPhase('downloading');
        setProgress(status.progress);
        setSpeed(status.speed);
      } else if (status.error) {
        setPhase('failed');
        setErrorMsg(status.error);
      } else if (status.progress >= 100) {
        setPhase('completed');
        setProgress(100);
      }
    } catch {
      // 轮询失败时静默处理
    }
  }, []);

  // 下载中时每 1s 轮询进度
  useEffect(() => {
    mountedRef.current = true;
    if (phase !== 'downloading') return;

    const timer = setInterval(() => {
      void pollDownloadStatus();
    }, 1000);

    return () => {
      clearInterval(timer);
    };
  }, [phase, pollDownloadStatus]);

  useEffect(() => {
    return () => {
      mountedRef.current = false;
    };
  }, []);

  /** 开始下载 */
  const handleDownload = useCallback(async () => {
    setPhase('downloading');
    setProgress(0);
    setErrorMsg('');

    try {
      await downloadModel(selectedSource);
      // 开始轮询进度
      void pollDownloadStatus();
    } catch (err) {
      if (mountedRef.current) {
        setPhase('failed');
        setErrorMsg(
          err instanceof Error ? err.message : '下载启动失败，请重试'
        );
      }
    }
  }, [selectedSource, pollDownloadStatus]);

  /** 加载模型 */
  const handleLoadModel = useCallback(async () => {
    setPhase('loading');
    setErrorMsg('');

    try {
      await loadModel();
      if (mountedRef.current) {
        setToastMessage('模型加载成功');
        setTimeout(() => {
          onModelLoaded();
        }, 1000);
      }
    } catch (err) {
      if (mountedRef.current) {
        setPhase('failed');
        setErrorMsg(
          err instanceof Error ? err.message : '模型加载失败，请检查 GPU 和模型文件'
        );
      }
    }
  }, [onModelLoaded]);

  /** 重试 */
  const handleRetry = useCallback(() => {
    setPhase('idle');
    setErrorMsg('');
    setProgress(0);
  }, []);

  /** 取消下载 */
  const handleCancelDownload = useCallback(async () => {
    try {
      await cancelDownload();
    } catch {
      // 取消请求失败时仍重置 UI（后端可能已退出）
    }
    if (mountedRef.current) {
      setPhase('idle');
      setProgress(0);
      setSpeed(0);
      setErrorMsg('');
    }
  }, []);

  return (
    <div
      className="flex flex-col bg-white w-full h-full"
      style={{
        borderRadius: '12px',
        boxShadow: '0 8px 32px rgba(0, 0, 0, 0.15)',
      }}
    >
      {/* 头部 */}
      <div className="px-8 pt-8 pb-4">
        <h1 className="text-xl font-bold" style={{ color: '#1D1D1F' }}>
          VoiceInput 已安装成功
        </h1>
        <p className="mt-2 text-sm leading-relaxed" style={{ color: '#86868B' }}>
          接下来需要下载语音识别模型（约 1.2 GB），下载后所有识别在本地完成，不需要联网。
        </p>
      </div>

      {/* 隐私说明 */}
      <div className="mx-8 mb-6 flex items-start gap-3 rounded-lg p-4" style={{ backgroundColor: '#F0F9EB' }}>
        <div style={{ color: '#34C759', flexShrink: 0 }}>
          <ShieldIcon size={20} color="#34C759" />
        </div>
        <div>
          <p className="text-sm font-medium" style={{ color: '#1D1D1F' }}>
            隐私无忧
          </p>
          <p className="mt-1 text-xs leading-relaxed" style={{ color: '#86868B' }}>
            你的语音数据不会被上传到任何服务器，所有识别在本地完成。
          </p>
        </div>
      </div>

      {/* 主内容区 */}
      <div className="flex-1 overflow-y-auto px-8 custom-scrollbar">
        {/* 下载源选择（idle/failed 状态显示） */}
        {(phase === 'idle' || phase === 'failed') && (
          <>
            <p className="mb-3 text-sm font-medium" style={{ color: '#1D1D1F' }}>
              选择下载源
            </p>
            <div className="space-y-2">
              {DOWNLOAD_SOURCES.map((source) => (
                <button
                  key={source.key}
                  type="button"
                  onClick={() => setSelectedSource(source.key)}
                  className="flex w-full items-center justify-between rounded-lg border p-4 text-left transition-colors"
                  style={{
                    borderColor: selectedSource === source.key ? '#3478F6' : '#E5E5EA',
                    backgroundColor:
                      selectedSource === source.key ? '#F0F6FF' : '#FFFFFF',
                  }}
                >
                  <div>
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-medium" style={{ color: '#1D1D1F' }}>
                        {source.label}
                      </span>
                      {source.recommended && (
                        <span
                          className="rounded px-1.5 py-0.5 text-xs font-medium"
                          style={{ backgroundColor: '#34C759', color: '#FFFFFF' }}
                        >
                          推荐
                        </span>
                      )}
                    </div>
                    <p className="mt-1 text-xs" style={{ color: '#86868B' }}>
                      {source.description}
                    </p>
                  </div>
                  {/* 选中标记 */}
                  <div
                    className="flex h-5 w-5 items-center justify-center rounded-full border-2"
                    style={{
                      borderColor: selectedSource === source.key ? '#3478F6' : '#D1D1D6',
                      flexShrink: 0,
                    }}
                  >
                    {selectedSource === source.key && (
                      <div
                        className="h-2.5 w-2.5 rounded-full"
                        style={{ backgroundColor: '#3478F6' }}
                      />
                    )}
                  </div>
                </button>
              ))}
            </div>

            {/* 错误信息 */}
            {phase === 'failed' && errorMsg && (
              <div
                className="mt-4 rounded-lg p-3 text-sm"
                style={{ backgroundColor: '#FFF0F0', color: '#FF3B30' }}
              >
                {errorMsg}
              </div>
            )}
          </>
        )}

        {/* 下载进度（downloading 状态显示） */}
        {phase === 'downloading' && (
          <div className="py-8">
            <div className="mb-4 flex items-center justify-center">
              <div className="flex items-center gap-2">
                <LoadingIcon size={20} color="#3478F6" />
                <span className="text-sm font-medium" style={{ color: '#1D1D1F' }}>
                  正在下载模型...
                </span>
              </div>
            </div>

            {/* 进度条 */}
            <div
              className="h-3 w-full overflow-hidden rounded-full"
              style={{ backgroundColor: '#F2F2F7' }}
            >
              <div
                className="h-full rounded-full transition-all duration-500"
                style={{
                  width: `${progress}%`,
                  background: 'linear-gradient(90deg, #3478F6 0%, #5AC8FA 100%)',
                }}
              />
            </div>

            {/* 进度信息 */}
            <div className="mt-3 flex items-center justify-between">
              <span className="text-sm font-medium" style={{ color: '#1D1D1F' }}>
                {progress.toFixed(1)}%
              </span>
              <span className="text-xs" style={{ color: '#86868B' }}>
                {formatSpeed(speed)}
              </span>
            </div>

            <p className="mt-6 text-center text-xs" style={{ color: '#86868B' }}>
              请保持网络连接，下载过程中请勿关闭窗口
            </p>
          </div>
        )}

        {/* 下载完成 → 加载模型 */}
        {phase === 'completed' && (
          <div className="py-8 text-center">
            <div
              className="mx-auto mb-4 flex h-16 w-16 items-center justify-center rounded-full"
              style={{ backgroundColor: '#34C759' }}
            >
              <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="#FFFFFF" strokeWidth="3" strokeLinecap="round" strokeLinejoin="round">
                <polyline points="20 6 9 17 4 12" />
              </svg>
            </div>
            <p className="text-base font-semibold" style={{ color: '#1D1D1F' }}>
              模型下载完成
            </p>
            <p className="mt-2 text-sm" style={{ color: '#86868B' }}>
              点击下方按钮将模型加载到 GPU，加载约需 5-20 秒
            </p>
          </div>
        )}

        {/* 模型加载中 */}
        {phase === 'loading' && (
          <div className="py-12 text-center">
            <div className="mb-4 flex justify-center">
              <LoadingIcon size={32} color="#3478F6" />
            </div>
            <p className="text-base font-semibold" style={{ color: '#1D1D1F' }}>
              正在加载语音识别模型...
            </p>
            <p className="mt-2 text-sm" style={{ color: '#86868B' }}>
              正在将模型加载到 GPU，请稍候
            </p>
          </div>
        )}

        {/* 加载失败 */}
        {phase === 'failed' && errorMsg && (
          <div className="mt-4">
            <button
              type="button"
              onClick={handleRetry}
              className="w-full rounded-lg py-2.5 text-sm font-medium transition-colors hover:bg-gray-100"
              style={{
                color: '#86868B',
                border: '1px solid #E5E5EA',
              }}
            >
              重试
            </button>
          </div>
        )}
      </div>

      {/* 底部操作按钮 */}
      <div className="px-8 py-6">
        {(phase === 'idle' || phase === 'failed') && (
          <button
            type="button"
            onClick={handleDownload}
            className="flex w-full items-center justify-center gap-2 rounded-lg py-3 text-sm font-medium text-white transition-opacity hover:opacity-90"
            style={{ backgroundColor: '#3478F6' }}
          >
            <DownloadIcon size={18} color="#FFFFFF" />
            {selectedSource === 'local' ? '使用本地模型' : '开始下载'}
          </button>
        )}

        {phase === 'completed' && (
          <button
            type="button"
            onClick={handleLoadModel}
            className="w-full rounded-lg py-3 text-sm font-medium text-white transition-opacity hover:opacity-90"
            style={{ backgroundColor: '#3478F6' }}
          >
            加载模型
          </button>
        )}

        {phase === 'downloading' && (
          <button
            type="button"
            onClick={handleCancelDownload}
            className="w-full rounded-lg py-3 text-sm font-medium transition-opacity hover:opacity-90"
            style={{
              color: '#FF3B30',
              border: '1px solid #FF3B30',
              backgroundColor: '#FFFFFF',
            }}
          >
            取消下载
          </button>
        )}

        {phase === 'loading' && (
          <div
            className="w-full rounded-lg py-3 text-center text-sm font-medium text-white"
            style={{ backgroundColor: '#D1D1D6' }}
          >
            加载中...
          </div>
        )}
      </div>

      {/* Toast */}
      {toastMessage && (
        <Toast
          message={toastMessage}
          duration={1500}
          onDismiss={() => setToastMessage('')}
        />
      )}
    </div>
  );
}

export default ModelDownload;
