/**
 * VoiceInput v2 — 快捷键捕获组件
 *
 * 点击按钮显示"按下新快捷键..."，监听 keydown 捕获组合键。
 * 按 Esc 取消捕获，返回原值。
 * 捕获到的组合键格式如 "alt+v"、"ctrl+shift+s"。
 */

import { useCallback, useEffect, useRef, useState } from 'react';

/** HotkeyCapture 组件 Props */
interface HotkeyCaptureProps {
  /** 当前快捷键值，如 "alt+v" */
  value: string;
  /** 快捷键变更回调 */
  onChange: (hotkey: string) => void;
}

/** 修饰键名称映射 */
const MODIFIER_MAP: Record<string, string> = {
  Control: 'ctrl',
  Alt: 'alt',
  Shift: 'shift',
  Meta: 'meta',
};

/** 普通键名称映射 */
const KEY_MAP: Record<string, string> = {
  ' ': 'space',
  ArrowUp: 'up',
  ArrowDown: 'down',
  ArrowLeft: 'left',
  ArrowRight: 'right',
  Escape: 'esc',
  Tab: 'tab',
  Backspace: 'backspace',
  Enter: 'enter',
  Delete: 'delete',
  Insert: 'insert',
  Home: 'home',
  End: 'end',
  PageUp: 'pageup',
  PageDown: 'pagedown',
};

/**
 * 将键盘事件转换为快捷键字符串
 * @param event 键盘事件
 * @returns 快捷键字符串如 "alt+v"，或 null（仅修饰键时）
 */
function eventToHotkey(event: KeyboardEvent): string | null {
  const parts: string[] = [];

  // 收集修饰键（保持固定顺序）
  if (event.ctrlKey) parts.push('ctrl');
  if (event.altKey) parts.push('alt');
  if (event.shiftKey) parts.push('shift');
  if (event.metaKey) parts.push('meta');

  // 获取主键
  const key = event.key;

  // 如果按下的是纯修饰键，不生成快捷键
  if (MODIFIER_MAP[key] !== undefined) {
    return null;
  }

  // 映射特殊键名
  let keyName = KEY_MAP[key];
  if (keyName === undefined) {
    // 单个字符直接小写
    if (key.length === 1) {
      keyName = key.toLowerCase();
    } else {
      // F1-F12 等功能键直接小写
      keyName = key.toLowerCase();
    }
  }

  parts.push(keyName);
  return parts.join('+');
}

/**
 * 将快捷键字符串格式化为显示文本
 * "alt+v" → "Alt + V"
 */
export function formatHotkey(hotkey: string): string {
  if (!hotkey) return '';
  return hotkey
    .split('+')
    .map((part) => {
      const trimmed = part.trim();
      if (trimmed.length === 1) {
        return trimmed.toUpperCase();
      }
      return trimmed.charAt(0).toUpperCase() + trimmed.slice(1);
    })
    .join(' + ');
}

/**
 * 快捷键捕获组件
 *
 * 点击后进入捕获模式，监听键盘事件。
 * 按下组合键后自动设置新值并退出捕获模式。
 * 按 Esc 取消捕获。
 */
export function HotkeyCapture({ value, onChange }: HotkeyCaptureProps): JSX.Element {
  const [capturing, setCapturing] = useState(false);
  const buttonRef = useRef<HTMLButtonElement>(null);

  const handleKeyDown = useCallback(
    (event: KeyboardEvent) => {
      // Esc 取消捕获
      if (event.key === 'Escape') {
        event.preventDefault();
        event.stopPropagation();
        setCapturing(false);
        return;
      }

      const hotkey = eventToHotkey(event);
      if (hotkey !== null) {
        event.preventDefault();
        event.stopPropagation();
        onChange(hotkey);
        setCapturing(false);
      }
    },
    [onChange]
  );

  useEffect(() => {
    if (capturing) {
      window.addEventListener('keydown', handleKeyDown, true);
      return () => {
        window.removeEventListener('keydown', handleKeyDown, true);
      };
    }
    return undefined;
  }, [capturing, handleKeyDown]);

  return (
    <button
      ref={buttonRef}
      type="button"
      onClick={() => setCapturing(true)}
      className="flex items-center justify-center rounded-lg border px-3 py-2 text-sm transition-colors"
      style={{
        minWidth: '180px',
        borderColor: capturing ? '#3478F6' : '#E5E5EA',
        backgroundColor: capturing ? '#F0F6FF' : '#FFFFFF',
        color: capturing ? '#3478F6' : '#1D1D1F',
      }}
    >
      {capturing ? '按下新快捷键...' : formatHotkey(value) || '点击设置快捷键'}
    </button>
  );
}

export default HotkeyCapture;
