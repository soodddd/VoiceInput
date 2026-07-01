/**
 * VoiceInput v2 — 悬浮窗组件
 * 交互模式：
 *   - 点击麦克风按钮：开始/停止录音（toggle模式）
 *   - 按住麦克风按钮：按住说话，松开停止
 */

import { useState, useRef, useCallback, useEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { MicIcon, StopIcon, CopyIcon, TrashIcon, SendIcon, GlobeIcon, CogIcon, ErrorIcon, SpinnerIcon, CloseIcon } from './Icons';
import { WaveformWidget } from './WaveformWidget';
import { ResultDisplay } from './ResultDisplay';
import type { AppStatus, Language } from '../types';
import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { invoke } from '@tauri-apps/api/core';

interface FloatingWindowProps {
  status: AppStatus;
  resultText: string;
  volumeLevel: number;
  recordingDuration: number;
  errorMessage: string;
  language: Language;
  onLanguageChange: (lang: Language) => void;
  onClearResult: () => void;
  onOpenSettings: () => void;
  manualStart: () => Promise<void>;
  manualStop: () => Promise<void>;
}

function formatDuration(seconds: number): string {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
}

export function FloatingWindow({
  status,
  resultText,
  volumeLevel,
  recordingDuration,
  errorMessage,
  language,
  onLanguageChange,
  onClearResult,
  onOpenSettings,
  manualStart,
  manualStop,
}: FloatingWindowProps) {
  const [copied, setCopied] = useState(false);

  const statusRef = useRef<AppStatus>(status);
  const buttonHeldRef = useRef(false);
  const clickTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    statusRef.current = status;
  }, [status]);

  useEffect(() => {
    return () => {
      if (clickTimeoutRef.current) {
        clearTimeout(clickTimeoutRef.current);
      }
    };
  }, []);

  const isRecording = status === 'recording';
  const isProcessing = status === 'processing';
  const isResult = status === 'result';
  const isIdle = status === 'idle';
  const isError = status === 'error';

  const toggleLanguage = useCallback(() => {
    const cycle: Language[] = ['auto', 'Chinese', 'English'];
    const idx = cycle.indexOf(language);
    const next = cycle[(idx + 1) % cycle.length];
    onLanguageChange(next);
  }, [language, onLanguageChange]);

  const langLabel = language === 'English' ? 'EN' : language === 'Chinese' ? '中' : 'Auto';
  const langColor = language === 'English' ? '#059669' : language === 'Chinese' ? '#2563EB' : '#8B5CF6';
  const langBg = language === 'English' ? 'rgba(5,150,105,0.1)' : language === 'Chinese' ? 'rgba(37,99,235,0.1)' : 'rgba(139,92,246,0.1)';

  const openSettings = useCallback(() => {
    onOpenSettings();
  }, [onOpenSettings]);

  const handleHide = useCallback(async () => {
    try {
      await getCurrentWindow().hide();
    } catch {
      // ignore
    }
  }, []);

  const handleCopy = useCallback(async () => {
    if (!resultText) return;
    try {
      await writeText(resultText);
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    } catch {
      // ignore
    }
  }, [resultText]);

  const handleRePaste = useCallback(async () => {
    if (!resultText) return;
    try {
      await invoke('paste_text', { text: resultText });
    } catch {
      // ignore
    }
  }, [resultText]);

  const handleErrorClick = useCallback(() => {
    if (isError) {
      onClearResult();
    }
  }, [isError, onClearResult]);

  const doStartRecording = useCallback(() => {
    if (statusRef.current === 'processing' || statusRef.current === 'recording') return;
    return manualStart();
  }, [manualStart]);

  const doStopRecording = useCallback(() => {
    if (statusRef.current === 'recording') {
      return manualStop();
    }
    return Promise.resolve();
  }, [manualStop]);

  const handleMicMouseDown = useCallback(() => {
    if (isProcessing) return;
    buttonHeldRef.current = true;

    if (isRecording) {
      void doStopRecording();
      buttonHeldRef.current = false;
      return;
    }

    if (isError || isResult || isIdle) {
      clickTimeoutRef.current = setTimeout(() => {
        if (buttonHeldRef.current && statusRef.current !== 'recording') {
          void doStartRecording();
        }
      }, 150);
    }
  }, [isProcessing, isRecording, isError, isResult, isIdle, doStartRecording, doStopRecording]);

  const handleMicMouseUp = useCallback(() => {
    if (clickTimeoutRef.current) {
      clearTimeout(clickTimeoutRef.current);
      clickTimeoutRef.current = null;
    }

    if (!buttonHeldRef.current) return;

    if (statusRef.current === 'recording') {
      void doStopRecording();
    } else if (!isProcessing && !isRecording) {
      void doStartRecording();
    }

    buttonHeldRef.current = false;
  }, [isProcessing, isRecording, doStartRecording, doStopRecording]);

  const handleMicMouseLeave = useCallback(() => {
    if (clickTimeoutRef.current) {
      clearTimeout(clickTimeoutRef.current);
      clickTimeoutRef.current = null;
    }
    if (buttonHeldRef.current && statusRef.current === 'recording') {
      void doStopRecording();
    }
    buttonHeldRef.current = false;
  }, [doStopRecording]);

  const micBgColor = isRecording
    ? '#EF4444'
    : isProcessing
    ? '#F59E0B'
    : isError
    ? '#EF4444'
    : '#2563EB';

  const micShadow = isRecording
    ? '0 0 0 8px rgba(239,68,68,0.2), 0 4px 20px rgba(239,68,68,0.4)'
    : isProcessing
    ? '0 0 0 6px rgba(245,158,11,0.15), 0 4px 16px rgba(245,158,11,0.3)'
    : isError
    ? '0 0 0 6px rgba(239,68,68,0.15), 0 4px 16px rgba(239,68,68,0.3)'
    : '0 4px 20px rgba(37,99,235,0.35)';

  const statusHint = isRecording
    ? '录音中 · 点击或松开停止'
    : isProcessing
    ? '正在识别...'
    : isError
    ? '识别失败，点击重试'
    : isResult
    ? '识别完成'
    : '点击麦克风或按 Alt+V 开始';

  return (
    <div
      className="flex flex-col w-full h-full select-none"
      style={{
        backgroundColor: '#FFFFFF',
        borderRadius: '14px',
        border: '1px solid rgba(0,0,0,0.08)',
        boxShadow: '0 8px 32px rgba(0,0,0,0.18), 0 2px 8px rgba(0,0,0,0.08)',
        overflow: 'hidden',
      }}
    >
      {/* 标题栏 — 可拖动 */}
      <div
        data-tauri-drag-region
        className="flex items-center justify-between px-2"
        style={{
          height: '38px',
          borderBottom: '1px solid rgba(0,0,0,0.05)',
          backgroundColor: '#FAFBFC',
          flexShrink: 0,
        }}
      >
        <div
          data-tauri-drag-region
          className="flex items-center gap-1.5 px-1"
          style={{ cursor: 'default' }}
        >
          <div
            className="flex items-center justify-center rounded-md"
            style={{ width: '20px', height: '20px', backgroundColor: '#2563EB', flexShrink: 0 }}
          >
            <MicIcon size={11} color="#FFFFFF" />
          </div>
          <span style={{ fontSize: '12px', fontWeight: 600, color: '#1E293B', userSelect: 'none' }}>
            VoiceInput
          </span>
          {isRecording && (
            <div className="flex items-center gap-1 ml-0.5">
              <span
                className="rounded-full"
                style={{
                  width: '6px', height: '6px',
                  backgroundColor: '#EF4444',
                  animation: 'pulse-dot 1s ease-in-out infinite',
                }}
              />
              <span
                style={{
                  fontSize: '11px', fontWeight: 600, color: '#EF4444',
                  fontVariantNumeric: 'tabular-nums',
                }}
              >
                {formatDuration(recordingDuration)}
              </span>
            </div>
          )}
        </div>
        <div className="flex items-center gap-0.5" data-tauri-drag-region="false">
          <button
            type="button"
            onClick={toggleLanguage}
            data-tauri-drag-region="false"
            className="flex items-center gap-1 rounded-md"
            style={{
              height: '26px', padding: '0 8px', fontSize: '11px', fontWeight: 600,
              color: langColor,
              backgroundColor: langBg,
              border: 'none', cursor: 'pointer',
              transition: 'all 0.15s',
            }}
            onMouseEnter={(e) => {
              const hoverBg = language === 'English' ? 'rgba(5,150,105,0.2)' : language === 'Chinese' ? 'rgba(37,99,235,0.2)' : 'rgba(139,92,246,0.2)';
              e.currentTarget.style.backgroundColor = hoverBg;
            }}
            onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = langBg; }}
            title={language === 'auto' ? 'Auto detect — click to switch to Chinese' : language === 'Chinese' ? 'Chinese — click to switch to English' : 'English — click to switch to Auto'}
          >
            <GlobeIcon size={11} color={langColor} />
            {langLabel}
          </button>
          <button
            type="button"
            onClick={openSettings}
            data-tauri-drag-region="false"
            className="flex items-center justify-center rounded-md"
            style={{
              width: '28px', height: '28px', border: 'none',
              backgroundColor: 'transparent', cursor: 'pointer',
              transition: 'background-color 0.15s',
            }}
            onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = 'rgba(0,0,0,0.06)'; }}
            onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = 'transparent'; }}
            title="设置"
          >
            <CogIcon size={15} color="#64748B" />
          </button>
          <button
            type="button"
            onClick={handleHide}
            data-tauri-drag-region="false"
            className="flex items-center justify-center rounded-md"
            style={{
              width: '28px', height: '28px', border: 'none',
              backgroundColor: 'transparent', cursor: 'pointer',
              transition: 'background-color 0.15s',
            }}
            onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = 'rgba(255,59,48,0.1)'; }}
            onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = 'transparent'; }}
            title="隐藏窗口 (托盘运行)"
          >
            <CloseIcon size={14} color="#8E8E93" />
          </button>
        </div>
      </div>

      {/* 主内容区 */}
      <div
        className="flex-1 flex flex-col items-center px-4"
        style={{
          minHeight: 0,
          paddingTop: isResult ? '8px' : '12px',
          paddingBottom: '8px',
          justifyContent: isResult ? 'flex-start' : 'center',
        }}
      >
        {/* 波形 — 录音/处理中显示 */}
        {(isRecording || isProcessing) && (
          <div className="mb-2" style={{ width: '100%', display: 'flex', justifyContent: 'center' }}>
            <WaveformWidget
              volumeLevel={volumeLevel}
              isActive={isRecording}
              barCount={17}
              height={32}
              color={isRecording ? '#EF4444' : '#F59E0B'}
            />
          </div>
        )}

        {/* 麦克风按钮 */}
        <button
          type="button"
          onMouseDown={handleMicMouseDown}
          onMouseUp={handleMicMouseUp}
          onMouseLeave={handleMicMouseLeave}
          onTouchStart={(e) => { e.preventDefault(); handleMicMouseDown(); }}
          onTouchEnd={(e) => { e.preventDefault(); handleMicMouseUp(); }}
          onClick={isError ? handleErrorClick : undefined}
          className="flex items-center justify-center rounded-full select-none"
          style={{
            width: isRecording ? '64px' : '56px',
            height: isRecording ? '64px' : '56px',
            backgroundColor: micBgColor,
            boxShadow: micShadow,
            border: 'none',
            cursor: isProcessing ? 'wait' : 'pointer',
            outline: 'none',
            flexShrink: 0,
            transition: 'width 0.15s, height 0.15s, background-color 0.2s, box-shadow 0.2s, transform 0.1s',
            WebkitTapHighlightColor: 'transparent',
            transform: isRecording ? 'scale(1)' : 'scale(1)',
          }}
          disabled={isProcessing}
        >
          {isProcessing ? (
            <SpinnerIcon size={24} color="#FFFFFF" />
          ) : isRecording ? (
            <StopIcon size={22} color="#FFFFFF" />
          ) : isError ? (
            <ErrorIcon size={22} color="#FFFFFF" />
          ) : (
            <MicIcon size={26} color="#FFFFFF" />
          )}
        </button>

        {/* 状态提示 */}
        <div
          className="mt-2 text-center"
          style={{
            fontSize: '11px', fontWeight: 500,
            color: isRecording ? '#EF4444' : isProcessing ? '#F59E0B' : isError ? '#EF4444' : isResult ? '#059669' : '#94A3B8',
            lineHeight: 1.3,
          }}
        >
          {statusHint}
        </div>

        {/* 错误信息 */}
        {isError && errorMessage && (
          <div
            className="mt-1.5 text-center rounded-lg px-2.5 py-1 w-full"
            style={{
              fontSize: '10px', color: '#DC2626',
              backgroundColor: 'rgba(239,68,68,0.06)',
              lineHeight: 1.4,
            }}
          >
            {errorMessage}
          </div>
        )}

        {/* 结果显示 */}
        {isResult && resultText && (
          <div className="w-full mt-2" style={{ flex: '1 1 auto', minHeight: 0 }}>
            <ResultDisplay text={resultText} maxHeight={80} />
          </div>
        )}
      </div>

      {/* 底部操作栏 — 仅在有结果时显示 */}
      {isResult && resultText && (
        <div
          className="flex items-center justify-center gap-2 px-3"
          style={{
            height: '42px',
            borderTop: '1px solid rgba(0,0,0,0.05)',
            backgroundColor: '#FAFBFC',
            flexShrink: 0,
          }}
        >
          <button
            type="button"
            onClick={onClearResult}
            className="flex items-center gap-1 rounded-lg"
            style={{
              height: '28px', padding: '0 12px', fontSize: '11px', fontWeight: 500,
              color: '#64748B', backgroundColor: '#F1F5F9',
              border: '1px solid rgba(0,0,0,0.06)', cursor: 'pointer',
              transition: 'all 0.15s',
            }}
            onMouseEnter={(e) => { e.currentTarget.style.backgroundColor = '#E2E8F0'; }}
            onMouseLeave={(e) => { e.currentTarget.style.backgroundColor = '#F1F5F9'; }}
            title="清空结果"
          >
            <TrashIcon size={12} color="#64748B" /> 清空
          </button>
          <button
            type="button"
            onClick={handleCopy}
            className="flex items-center gap-1 rounded-lg"
            style={{
              height: '28px', padding: '0 12px', fontSize: '11px', fontWeight: 500,
              color: copied ? '#FFFFFF' : '#2563EB',
              backgroundColor: copied ? '#059669' : 'rgba(37,99,235,0.08)',
              border: '1px solid',
              borderColor: copied ? '#059669' : 'rgba(37,99,235,0.15)',
              cursor: 'pointer',
              transition: 'all 0.15s',
            }}
            title="复制到剪贴板"
          >
            <CopyIcon size={12} color={copied ? '#FFFFFF' : '#2563EB'} />
            {copied ? '已复制' : '复制'}
          </button>
          <button
            type="button"
            onClick={handleRePaste}
            className="flex items-center gap-1 rounded-lg"
            style={{
              height: '28px', padding: '0 16px', fontSize: '11px', fontWeight: 600,
              color: '#FFFFFF', backgroundColor: '#2563EB',
              border: 'none', cursor: 'pointer',
              boxShadow: '0 2px 6px rgba(37,99,235,0.3)',
              transition: 'all 0.15s',
            }}
            onMouseEnter={(e) => {
              e.currentTarget.style.backgroundColor = '#1D4ED8';
              e.currentTarget.style.boxShadow = '0 2px 8px rgba(37,99,235,0.4)';
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.backgroundColor = '#2563EB';
              e.currentTarget.style.boxShadow = '0 2px 6px rgba(37,99,235,0.3)';
            }}
            title="粘贴到光标处"
          >
            <SendIcon size={12} color="#FFFFFF" /> 插入
          </button>
        </div>
      )}

      <style>{`
        @keyframes pulse-dot {
          0%, 100% { opacity: 1; }
          50% { opacity: 0.3; }
        }
      `}</style>
    </div>
  );
}

export default FloatingWindow;
