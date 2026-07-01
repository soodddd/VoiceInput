/**
 * VoiceInput v2 — 设置面板（4 Tab）
 *
 * 500×540px 白色背景，4 个 Tab 页：
 * 1. 麦克风：设备下拉 + 测试按钮 + 结果显示
 * 2. 快捷键：录音快捷键捕获 + 语言快捷键捕获
 * 3. 音频：采样率 + 归一化 + 裁剪静音 + 静音阈值
 * 4. 高级：服务器地址 + 粘贴延迟 + 恢复剪贴板 + 最大录音时长 + 请求超时
 *
 * 底部 [保存] [取消] 按钮，保存时调 invoke('save_config', {config})。
 */

import { useEffect, useState, useCallback, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { useSettings } from '../hooks/useSettings';
import { HotkeyCapture } from './HotkeyCapture';
import { Toast } from './Toast';
import { RefreshIcon } from './Icons';
import { getDevices, startRecording, stopRecording, setModelStrategy } from '../utils/api';
import type { AudioDevice } from '../types';

/** 模型策略选项 */
const MODEL_STRATEGIES = [
  { value: 'balanced', label: '平衡模式（空闲 30 分钟释放）' },
  { value: 'fast', label: '性能优先（常驻显存）' },
  { value: 'memory', label: '省显存（每次识别后释放）' },
  { value: 'accurate', label: '质量优先（最大精度）' },
];

/** 标点模式选项 */
const PUNCTUATION_MODES = [
  { value: 'raw', label: '原始输出' },
  { value: 'simple', label: '简单标点（自动补充）' },
  { value: 'input_method', label: '输入法模式（减少标点）' },
];

/** SettingsDialog 组件 Props */
interface SettingsDialogProps {
  /** 保存回调 */
  onSave: () => void;
  /** 取消回调 */
  onCancel: () => void;
}

/** Tab 页定义 */
type TabKey = 'microphone' | 'hotkey' | 'audio' | 'advanced' | 'terms';

interface TabDef {
  key: TabKey;
  label: string;
}

const TABS: TabDef[] = [
  { key: 'microphone', label: '麦克风' },
  { key: 'hotkey', label: '快捷键' },
  { key: 'audio', label: '音频' },
  { key: 'advanced', label: '高级' },
  { key: 'terms', label: '术语' },
];

/** 采样率选项 */
const SAMPLE_RATES = [8000, 16000, 22050, 44100, 48000];

/** Toggle 开关组件 */
function Toggle({
  checked,
  onChange,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
}): JSX.Element {
  return (
    <button
      type="button"
      onClick={() => onChange(!checked)}
      className="relative inline-flex h-6 w-11 items-center rounded-full transition-colors"
      style={{
        backgroundColor: checked ? '#34C759' : '#E5E5EA',
      }}
    >
      <span
        className="inline-block h-5 w-5 transform rounded-full bg-white shadow transition-transform"
        style={{
          transform: checked ? 'translateX(22px)' : 'translateX(2px)',
        }}
      />
    </button>
  );
}

/** 设置行标签组件 */
function SettingRow({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}): JSX.Element {
  return (
    <div className="flex items-center gap-3 py-3">
      <div className="min-w-0 flex-1">
        <div className="text-sm font-medium" style={{ color: '#1D1D1F' }}>
          {label}
        </div>
        {hint && (
          <div className="mt-0.5 text-xs leading-relaxed" style={{ color: '#86868B' }}>
            {hint}
          </div>
        )}
      </div>
      <div className="flex flex-shrink-0 items-center gap-2">{children}</div>
    </div>
  );
}

/**
 * 设置面板组件
 */
export function SettingsDialog({ onSave, onCancel }: SettingsDialogProps): JSX.Element {
  const { config, updateConfig, saveSettings, loading } = useSettings();
  const [activeTab, setActiveTab] = useState<TabKey>('microphone');
  const [toastMessage, setToastMessage] = useState('');
  const [devices, setDevices] = useState<AudioDevice[]>([]);
  const [testing, setTesting] = useState(false);
  const [testResult, setTestResult] = useState('');
  const maxLevelRef = useRef(0);

  // 自定义术语编辑状态
  const [termWrong, setTermWrong] = useState('');
  const [termCorrect, setTermCorrect] = useState('');

  /** 加载设备列表 */
  const loadDevices = useCallback(async () => {
    try {
      const list = await getDevices();
      setDevices(list);
    } catch {
      // get_devices 命令可能未实现，使用默认选项
      setDevices([]);
    }
  }, []);

  useEffect(() => {
    void loadDevices();
  }, [loadDevices]);

  /** 测试麦克风（录音 2 秒后显示最大音量） */
  const handleTestMic = useCallback(async () => {
    setTesting(true);
    setTestResult('');
    maxLevelRef.current = 0;

    try {
      await startRecording();

      // 监听 audio-level 事件 2 秒
      const unlistenFn = await listen<number>('audio-level', (event) => {
        if (event.payload > maxLevelRef.current) {
          maxLevelRef.current = event.payload;
        }
      });

      // 等待 2 秒
      await new Promise<void>((resolve) => setTimeout(resolve, 2000));

      // 停止录音
      try {
        await stopRecording();
      } catch {
        // 忽略停止录音错误
      }

      unlistenFn();

      const maxLevel = maxLevelRef.current;
      if (maxLevel > 0.01) {
        const percent = Math.round(maxLevel * 100);
        setTestResult(`麦克风正常，音量 ${percent}%`);
      } else {
        setTestResult('未检测到声音，请检查麦克风连接');
      }
    } catch {
      setTestResult('麦克风测试失败，请检查设备');
    } finally {
      setTesting(false);
    }
  }, []);

  /** 保存配置 */
  const handleSave = useCallback(async () => {
    const success = await saveSettings();
    if (success) {
      setToastMessage('设置已保存');
      setTimeout(() => {
        onSave();
      }, 800);
    } else {
      setToastMessage('保存失败，请重试');
    }
  }, [saveSettings, onSave]);

  /** 取消 */
  const handleCancel = useCallback(() => {
    onCancel();
  }, [onCancel]);

  /** 添加自定义术语 */
  const handleAddTerm = useCallback(() => {
    const wrong = termWrong.trim();
    const correct = termCorrect.trim();
    if (!wrong || !correct) {
      setToastMessage('请填写误识别文本和正确文本');
      return;
    }
    const next = { ...config.custom_terms, [wrong]: correct };
    updateConfig('custom_terms', next);
    setTermWrong('');
    setTermCorrect('');
  }, [termWrong, termCorrect, config.custom_terms, updateConfig]);

  /** 删除自定义术语 */
  const handleRemoveTerm = useCallback(
    (wrong: string) => {
      const next = { ...config.custom_terms };
      delete next[wrong];
      updateConfig('custom_terms', next);
    },
    [config.custom_terms, updateConfig]
  );

  // 加载中
  if (loading) {
    return (
      <div
        className="flex items-center justify-center bg-white w-full h-full"
        style={{ borderRadius: '12px' }}
      >
        <div className="text-sm" style={{ color: '#86868B' }}>
          正在加载设置...
        </div>
      </div>
    );
  }

  return (
    <div
      className="flex h-full w-full flex-col overflow-hidden"
      style={{
        backgroundColor: '#FFFFFF',
        borderRadius: '12px',
      }}
    >
      {/* 标题栏 */}
      <div
        className="flex items-center justify-between border-b px-5 py-3.5"
        style={{ borderColor: '#E5E5EA' }}
      >
        <h2 className="text-base font-semibold" style={{ color: '#1D1D1F' }}>
          设置
        </h2>
      </div>

      {/* Tab 导航 */}
      <div className="flex border-b" style={{ borderColor: '#E5E5EA' }}>
        {TABS.map((tab) => (
          <button
            key={tab.key}
            type="button"
            onClick={() => setActiveTab(tab.key)}
            className="flex-1 px-2 py-2.5 text-sm font-medium transition-colors"
            style={{
              color: activeTab === tab.key ? '#3478F6' : '#86868B',
              borderBottom: activeTab === tab.key
                ? '2px solid #3478F6'
                : '2px solid transparent',
              marginBottom: '-1px',
            }}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {/* Tab 内容区 */}
      <div
        className="custom-scrollbar flex-1 overflow-y-auto overflow-x-hidden px-5"
        style={{ backgroundColor: '#FFFFFF' }}
      >
        {/* Tab 1: 麦克风 */}
        {activeTab === 'microphone' && (
          <div className="py-4">
            <SettingRow
              label="输入设备"
              hint="选择「系统默认」将使用 Windows 设置的默认麦克风"
            >
              <select
                value={config.input_device === null ? -1 : config.input_device}
                onChange={(e) => {
                  const val = parseInt(e.target.value, 10);
                  updateConfig('input_device', val === -1 ? null : val);
                }}
                className="max-w-[200px] rounded-lg border px-3 py-2 text-sm"
                style={{
                  borderColor: '#E5E5EA',
                  color: '#1D1D1F',
                }}
              >
                <option value={-1}>系统默认</option>
                {devices.map((device) => (
                  <option key={device.index} value={device.index}>
                    {device.name}
                  </option>
                ))}
              </select>
              <button
                type="button"
                onClick={loadDevices}
                className="flex items-center justify-center rounded-lg p-2 transition-colors hover:bg-gray-100"
                style={{ color: '#86868B' }}
                title="刷新设备列表"
              >
                <RefreshIcon size={16} color="#86868B" />
              </button>
            </SettingRow>

            <div className="my-4" style={{ borderTop: '1px solid #F2F2F7' }} />

            <SettingRow label="麦克风测试" hint="点击后录音 2 秒，检测麦克风是否正常">
              <button
                type="button"
                onClick={handleTestMic}
                disabled={testing}
                className="rounded-lg px-4 py-2 text-sm font-medium text-white transition-opacity disabled:opacity-50"
                style={{ backgroundColor: '#3478F6' }}
              >
                {testing ? '测试中...' : '测试麦克风（2秒）'}
              </button>
            </SettingRow>

            {testResult && (
              <div
                className="mt-2 rounded-lg p-3 text-sm"
                style={{
                  backgroundColor: '#F5F5F7',
                  color: '#1D1D1F',
                }}
              >
                {testResult}
              </div>
            )}
          </div>
        )}

        {/* Tab 2: 快捷键 */}
        {activeTab === 'hotkey' && (
          <div className="py-4">
            <SettingRow
              label="录音快捷键"
              hint="按住此快捷键说话，松开后自动识别并粘贴"
            >
              <HotkeyCapture
                value={config.hotkey}
                onChange={(v) => updateConfig('hotkey', v)}
              />
            </SettingRow>

            <div className="my-4" style={{ borderTop: '1px solid #F2F2F7' }} />

            <SettingRow
              label="语言切换快捷键"
              hint="按下此快捷键在 Auto / 中文 / 英文 之间循环切换"
            >
              <HotkeyCapture
                value={config.language_hotkey}
                onChange={(v) => updateConfig('language_hotkey', v)}
              />
            </SettingRow>

            <div className="mt-6 rounded-lg p-3" style={{ backgroundColor: '#F5F5F7' }}>
              <p className="text-xs leading-relaxed" style={{ color: '#86868B' }}>
                提示：点击按钮后按下新的快捷键组合，按 Esc 取消。
                建议使用 Alt 或 Ctrl 组合键，避免与系统快捷键冲突。
              </p>
            </div>
          </div>
        )}

        {/* Tab 3: 音频 */}
        {activeTab === 'audio' && (
          <div className="py-4">
            <SettingRow label="采样率" hint="推荐 16000 Hz，与 ASR 模型匹配">
              <select
                value={config.sample_rate}
                onChange={(e) => updateConfig('sample_rate', parseInt(e.target.value, 10))}
                className="w-28 rounded-lg border px-3 py-2 text-sm"
                style={{
                  borderColor: '#E5E5EA',
                  color: '#1D1D1F',
                }}
              >
                {SAMPLE_RATES.map((rate) => (
                  <option key={rate} value={rate}>
                    {rate} Hz
                  </option>
                ))}
              </select>
            </SettingRow>

            <div className="my-2" style={{ borderTop: '1px solid #F2F2F7' }} />

            <SettingRow label="音量归一化" hint="自动调整录音音量到标准级别">
              <Toggle
                checked={config.normalize_audio}
                onChange={(v) => updateConfig('normalize_audio', v)}
              />
            </SettingRow>

            <div className="my-2" style={{ borderTop: '1px solid #F2F2F7' }} />

            <SettingRow label="裁剪静音" hint="自动去除录音头尾的静音段">
              <Toggle
                checked={config.trim_silence}
                onChange={(v) => updateConfig('trim_silence', v)}
              />
            </SettingRow>

            <div className="my-2" style={{ borderTop: '1px solid #F2F2F7' }} />

            <SettingRow
              label="静音阈值"
              hint={`低于此分贝值的音频视为静音（当前: ${config.silence_threshold_db} dB）`}
            >
              <input
                type="range"
                min={-100}
                max={-10}
                step={1}
                value={config.silence_threshold_db}
                onChange={(e) =>
                  updateConfig('silence_threshold_db', parseFloat(e.target.value))
                }
                className="w-32"
                style={{ accentColor: '#3478F6' }}
              />
            </SettingRow>
          </div>
        )}

        {/* Tab 4: 高级 */}
        {activeTab === 'advanced' && (
          <div className="py-4">
            <SettingRow label="服务器地址" hint="ASR 后端地址，通常不需要修改">
              <input
                type="text"
                value={config.server_url}
                onChange={(e) => updateConfig('server_url', e.target.value)}
                className="w-44 rounded-lg border px-3 py-2 text-sm"
                style={{
                  borderColor: '#E5E5EA',
                  color: '#1D1D1F',
                }}
              />
            </SettingRow>

            <div className="my-2" style={{ borderTop: '1px solid #F2F2F7' }} />

            <SettingRow
              label="粘贴延迟"
              hint={`写入剪贴板后等待多久再粘贴（当前: ${config.paste_delay_ms} ms）`}
            >
              <input
                type="range"
                min={0}
                max={2000}
                step={100}
                value={config.paste_delay_ms}
                onChange={(e) =>
                  updateConfig('paste_delay_ms', parseInt(e.target.value, 10))
                }
                className="w-32"
                style={{ accentColor: '#3478F6' }}
              />
            </SettingRow>

            <div className="my-2" style={{ borderTop: '1px solid #F2F2F7' }} />

            <SettingRow label="恢复剪贴板" hint="粘贴后恢复原来的剪贴板内容">
              <Toggle
                checked={config.clipboard_restore}
                onChange={(v) => updateConfig('clipboard_restore', v)}
              />
            </SettingRow>

            <div className="my-2" style={{ borderTop: '1px solid #F2F2F7' }} />

            <SettingRow
              label="最大录音时长"
              hint={`超过此时长自动停止录音（当前: ${config.max_record_sec} 秒）`}
            >
              <input
                type="range"
                min={10}
                max={600}
                step={10}
                value={config.max_record_sec}
                onChange={(e) =>
                  updateConfig('max_record_sec', parseInt(e.target.value, 10))
                }
                className="w-32"
                style={{ accentColor: '#3478F6' }}
              />
            </SettingRow>

            <div className="my-2" style={{ borderTop: '1px solid #F2F2F7' }} />

            <SettingRow
              label="请求超时"
              hint={`识别请求超时时间（当前: ${config.request_timeout_sec} 秒）`}
            >
              <input
                type="range"
                min={10}
                max={600}
                step={10}
                value={config.request_timeout_sec}
                onChange={(e) =>
                  updateConfig('request_timeout_sec', parseInt(e.target.value, 10))
                }
                className="w-32"
                style={{ accentColor: '#3478F6' }}
              />
            </SettingRow>

            <div className="my-2" style={{ borderTop: '1px solid #F2F2F7' }} />

            <SettingRow
              label="模型策略"
              hint="控制模型加载/释放策略，影响显存占用和响应速度"
            >
              <select
                value={config.model_strategy}
                onChange={(e) => {
                  updateConfig('model_strategy', e.target.value);
                  void setModelStrategy(e.target.value).catch(() => {});
                }}
                className="max-w-[200px] rounded-lg border px-3 py-2 text-sm"
                style={{ borderColor: '#E5E5EA', color: '#1D1D1F' }}
              >
                {MODEL_STRATEGIES.map((s) => (
                  <option key={s.value} value={s.value}>
                    {s.label}
                  </option>
                ))}
              </select>
            </SettingRow>

            <div className="my-2" style={{ borderTop: '1px solid #F2F2F7' }} />

            <SettingRow
              label="标点模式"
              hint="控制识别结果中的标点符号处理方式"
            >
              <select
                value={config.punctuation_mode}
                onChange={(e) => updateConfig('punctuation_mode', e.target.value)}
                className="max-w-[200px] rounded-lg border px-3 py-2 text-sm"
                style={{ borderColor: '#E5E5EA', color: '#1D1D1F' }}
              >
                {PUNCTUATION_MODES.map((m) => (
                  <option key={m.value} value={m.value}>
                    {m.label}
                  </option>
                ))}
              </select>
            </SettingRow>

            <div className="my-2" style={{ borderTop: '1px solid #F2F2F7' }} />

            <SettingRow label="中英自动空格" hint="在中文和英文之间自动添加空格，改善排版">
              <Toggle
                checked={config.auto_space_zh_en}
                onChange={(v) => updateConfig('auto_space_zh_en', v)}
              />
            </SettingRow>

            <div className="my-2" style={{ borderTop: '1px solid #F2F2F7' }} />

            <SettingRow label="VAD 静音自动停止" hint="检测到持续静音 2 秒后自动停止录音">
              <Toggle
                checked={config.vad_enabled}
                onChange={(v) => updateConfig('vad_enabled', v)}
              />
            </SettingRow>

            <div className="my-2" style={{ borderTop: '1px solid #F2F2F7' }} />

            <SettingRow label="开机自启" hint="Windows 开机后自动启动 VoiceInput">
              <Toggle
                checked={config.auto_start}
                onChange={(v) => updateConfig('auto_start', v)}
              />
            </SettingRow>
          </div>
        )}

        {/* Tab 5: 自定义术语 */}
        {activeTab === 'terms' && (
          <div className="py-4">
            <p className="mb-2 text-sm" style={{ color: '#86868B' }}>
              添加 ASR 误识别 → 正确文本的映射，识别结果会自动替换。
            </p>

            {/* 添加新术语表单 */}
            <div className="mb-4 rounded-lg p-3" style={{ backgroundColor: '#F5F5F7' }}>
              <div className="flex flex-col gap-2">
                <input
                  type="text"
                  value={termWrong}
                  onChange={(e) => setTermWrong(e.target.value)}
                  placeholder="误识别文本（如：派森）"
                  className="w-full rounded-lg border px-3 py-2 text-sm"
                  style={{ borderColor: '#E5E5EA', color: '#1D1D1F' }}
                />
                <input
                  type="text"
                  value={termCorrect}
                  onChange={(e) => setTermCorrect(e.target.value)}
                  placeholder="正确文本（如：Python）"
                  className="w-full rounded-lg border px-3 py-2 text-sm"
                  style={{ borderColor: '#E5E5EA', color: '#1D1D1F' }}
                />
                <button
                  type="button"
                  onClick={handleAddTerm}
                  disabled={!termWrong.trim() || !termCorrect.trim()}
                  className="rounded-lg py-2 text-sm font-medium text-white transition-opacity disabled:opacity-50"
                  style={{ backgroundColor: '#3478F6' }}
                >
                  添加术语
                </button>
              </div>
            </div>

            {/* 已有术语列表 */}
            <div className="space-y-2">
              {Object.entries(config.custom_terms).length === 0 ? (
                <p
                  className="py-6 text-center text-xs"
                  style={{ color: '#86868B' }}
                >
                  暂无自定义术语
                </p>
              ) : (
                Object.entries(config.custom_terms).map(([wrong, correct]) => (
                  <div
                    key={wrong}
                    className="flex items-center gap-2 rounded-lg border p-3"
                    style={{ borderColor: '#E5E5EA' }}
                  >
                    <div className="flex flex-1 items-center gap-2 overflow-hidden text-sm">
                      <span
                        className="truncate"
                        style={{ color: '#FF3B30' }}
                        title={wrong}
                      >
                        {wrong}
                      </span>
                      <span style={{ color: '#86868B' }}>→</span>
                      <span
                        className="truncate"
                        style={{ color: '#1D1D1F' }}
                        title={correct}
                      >
                        {correct}
                      </span>
                    </div>
                    <button
                      type="button"
                      onClick={() => handleRemoveTerm(wrong)}
                      className="flex-shrink-0 rounded px-2 py-1 text-xs transition-colors hover:bg-gray-100"
                      style={{ color: '#FF3B30' }}
                      title="删除此术语"
                    >
                      删除
                    </button>
                  </div>
                ))
              )}
            </div>

            <div className="mt-4 rounded-lg p-3" style={{ backgroundColor: '#F5F5F7' }}>
              <p className="text-xs leading-relaxed" style={{ color: '#86868B' }}>
                提示：较长的误识别文本优先匹配。术语在 ASR 内置术语修正之后应用。
              </p>
            </div>
          </div>
        )}
      </div>

      {/* 底部按钮 */}
      <div
        className="flex items-center justify-end gap-3 border-t px-5 py-3"
        style={{ borderColor: '#E5E5EA' }}
      >
        <button
          type="button"
          onClick={handleCancel}
          className="rounded-lg px-4 py-2 text-sm font-medium transition-colors hover:bg-gray-100"
          style={{
            color: '#86868B',
            border: '1px solid #E5E5EA',
          }}
        >
          取消
        </button>
        <button
          type="button"
          onClick={handleSave}
          className="rounded-lg px-4 py-2 text-sm font-medium text-white transition-opacity hover:opacity-90"
          style={{ backgroundColor: '#3478F6' }}
        >
          保存
        </button>
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

export default SettingsDialog;
