/**
 * VoiceInput v2 — 波形动画组件
 *
 * 柱状波形，柱数/颜色/高度可配置。
 * 录音时柱条跟随音量级别动态变化高度。
 */

import { useMemo } from 'react';

interface WaveformWidgetProps {
  /** 音量级别 0.0 ~ 1.0 */
  volumeLevel: number;
  /** 是否处于活跃状态（录音中） */
  isActive: boolean;
  /** 柱条数量，默认 9 */
  barCount?: number;
  /** 容器高度（px），默认 40 */
  height?: number;
  /** 活跃状态颜色，默认根据音量自动变色 */
  color?: string;
}

const BAR_WIDTH = 4;
const BAR_GAP = 5;
const MIN_HEIGHT = 4;

/**
 * 根据音量级别获取波形条颜色
 */
function getAutoColor(level: number): string {
  if (level < 0.4) return '#3478F6';
  if (level < 0.7) return '#FFCC00';
  return '#FF3B30';
}

/**
 * 生成柱条高度乘数（拱形分布）
 */
function generateMultipliers(count: number): number[] {
  const result: number[] = [];
  const mid = (count - 1) / 2;
  for (let i = 0; i < count; i++) {
    const dist = Math.abs(i - mid) / mid;
    result.push(1 - dist * 0.65);
  }
  return result;
}

export function WaveformWidget({
  volumeLevel,
  isActive,
  barCount = 9,
  height = 40,
  color,
}: WaveformWidgetProps): JSX.Element {
  const maxHeight = height;
  const multipliers = useMemo(() => generateMultipliers(barCount), [barCount]);

  const containerWidth = barCount * BAR_WIDTH + (barCount - 1) * BAR_GAP;

  const barColor = isActive
    ? (color || getAutoColor(volumeLevel))
    : '#D1D1D6';

  const barHeights = useMemo(() => {
    return multipliers.map((multiplier) => {
      if (!isActive) return MIN_HEIGHT;
      const scaled = Math.max(0, Math.min(1, volumeLevel * multiplier));
      const h = MIN_HEIGHT + (maxHeight - MIN_HEIGHT) * scaled;
      return Math.max(MIN_HEIGHT, Math.round(h));
    });
  }, [volumeLevel, isActive, multipliers, maxHeight]);

  return (
    <div
      className="flex items-center justify-center"
      style={{ width: containerWidth, height: maxHeight }}
    >
      {barHeights.map((h, index) => (
        <div
          key={index}
          style={{
            width: `${BAR_WIDTH}px`,
            height: `${h}px`,
            backgroundColor: barColor,
            borderRadius: '2px',
            marginLeft: index === 0 ? 0 : `${BAR_GAP}px`,
            transition: 'height 0.08s ease-out, background-color 0.15s ease-out',
          }}
        />
      ))}
    </div>
  );
}

export default WaveformWidget;
