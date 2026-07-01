/**
 * VoiceInput v2 — Toast 通知组件
 *
 * 底部浮现的黑色圆角提示，自动消失。
 * 用于显示"已粘贴到光标位置"、"已复制到剪贴板"等操作反馈。
 */

import { useEffect, useState } from 'react';

/** Toast 组件 Props */
interface ToastProps {
  /** 提示消息文本 */
  message: string;
  /** 显示时长（毫秒），默认 2000 */
  duration?: number;
  /** 消失时的回调 */
  onDismiss: () => void;
}

/**
 * Toast 通知组件
 *
 * 在容器底部居中显示黑色圆角提示，自动在指定时长后消失。
 * 支持淡入淡出动画。
 */
export function Toast({ message, duration = 2000, onDismiss }: ToastProps): JSX.Element {
  const [visible, setVisible] = useState(true);

  useEffect(() => {
    const timer = setTimeout(() => {
      setVisible(false);
      // 等待淡出动画完成后再回调
      setTimeout(onDismiss, 200);
    }, duration);

    return () => clearTimeout(timer);
  }, [duration, onDismiss]);

  if (!message) {
    return <></>;
  }

  return (
    <div
      className="pointer-events-none fixed bottom-6 left-1/2 z-50 -translate-x-1/2"
      style={{
        opacity: visible ? 1 : 0,
        transition: 'opacity 0.2s ease-in-out',
      }}
    >
      <div
        className="rounded-lg px-4 py-2 text-sm font-medium text-white shadow-lg"
        style={{ backgroundColor: 'rgba(0, 0, 0, 0.78)' }}
      >
        {message}
      </div>
    </div>
  );
}

export default Toast;
