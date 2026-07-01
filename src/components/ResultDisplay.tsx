/**
 * VoiceInput v2 — 识别结果显示组件
 *
 * 展示 ASR 识别文本，支持滚动和选中。
 */

import { useRef, useEffect } from 'react';

interface ResultDisplayProps {
  text: string;
  maxHeight?: number;
}

export function ResultDisplay({ text, maxHeight = 120 }: ResultDisplayProps): JSX.Element {
  const textRef = useRef<HTMLDivElement>(null);

  // 新文本出现时自动滚动到底部
  useEffect(() => {
    if (textRef.current) {
      textRef.current.scrollTop = 0;
    }
  }, [text]);

  return (
    <div
      ref={textRef}
      className="w-full rounded-xl px-3 py-2.5"
      style={{
        backgroundColor: 'rgba(241,245,249,0.8)',
        border: '1px solid rgba(0,0,0,0.05)',
        maxHeight: `${maxHeight}px`,
        overflowY: 'auto',
        lineHeight: 1.6,
        fontSize: '13px',
        color: '#1E293B',
        wordBreak: 'break-word',
        whiteSpace: 'pre-wrap',
        userSelect: 'text',
        cursor: 'text',
      }}
    >
      {text}
    </div>
  );
}

export default ResultDisplay;
