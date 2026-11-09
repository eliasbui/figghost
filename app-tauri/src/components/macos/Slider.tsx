import { useEffect, useRef, useState } from 'react';
import type { CSSProperties, ReactNode } from 'react';
import { change } from '@/stores/app';
import { useSetting } from '@/stores/hooks';
import { Row } from './Layout';

/** A track with a thumb of glass, and what it says beside it. The track is a native range input, so it has every key and the
 *  accessibility of one; the fill under the thumb is a gradient that stops at `--p`. */
export function Slider({
  value, min, max, step, onChange, label, format, onPress, onRelease, disabled,
}: {
  value: number;
  min: number;
  max: number;
  step: number;
  onChange: (value: number) => void;
  label: string;
  format?: (value: number) => string;
  onPress?: () => void;
  onRelease?: () => void;
  disabled?: boolean;
}) {
  const style = { '--p': `${Math.min(100, Math.max(0, ((value - min) / (max - min)) * 100))}%` } as CSSProperties;
  return (
    <div className="slider">
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        style={style}
        aria-label={label}
        disabled={disabled}
        onPointerDown={onPress}
        onPointerUp={onRelease}
        onChange={(e) => onChange(+e.target.value)}
      />
      {format ? <output aria-live="off">{format(value)}</output> : null}
    </div>
  );
}

export function SliderRow({
  path, title, desc, min, max, step, format, when, onInput,
}: { path: string; title: string; desc?: ReactNode; min: number; max: number; step: number; format: (value: number) => string; when?: boolean; onInput?: () => void }) {
  const stored = useSetting<number>(path);
  // While the thumb is held, what it shows is its own: a state the app sent a moment ago must not pull it back.
  const [held, setHeld] = useState<number | null>(null);
  const dragging = useRef(false);
  useEffect(() => {
    const release = () => {
      dragging.current = false;
      setHeld(null);
    };
    window.addEventListener('pointerup', release);
    return () => window.removeEventListener('pointerup', release);
  }, []);
  const value = held ?? stored ?? min;
  return (
    <Row title={title} desc={desc} when={when}>
      <Slider
        value={value}
        min={min}
        max={max}
        step={step}
        label={title}
        format={format}
        onPress={() => {
          dragging.current = true;
        }}
        onChange={(next) => {
          if (dragging.current) setHeld(next);
          change(path, next);
          onInput?.();
        }}
      />
    </Row>
  );
}
