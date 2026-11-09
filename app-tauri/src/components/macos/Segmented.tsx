import { useId } from 'react';
import type { KeyboardEvent, ReactNode } from 'react';
import { motion } from 'motion/react';
import { cn } from '@/lib/cn';
import { change } from '@/stores/app';
import { useSetting, useT } from '@/stores/hooks';
import { Row } from './Layout';

export interface Choice<V extends string | number = string> {
  value: V;
  label: ReactNode;
  /** The words for the reader of the screen when the label is only a picture. */
  name?: string;
}

/** A well with a thumb of glass that glides to the choice. A value that is none of the choices chooses none. */
export function Segmented<V extends string | number>({
  options, value, onPick, small, label, fill,
}: { options: Choice<V>[]; value: V | undefined; onPick: (value: V) => void; small?: boolean; label?: string; fill?: boolean }) {
  const id = useId();
  const at = options.findIndex((o) => o.value === value);
  const onKey = (e: KeyboardEvent) => {
    const step = e.key === 'ArrowRight' || e.key === 'ArrowDown' ? 1 : e.key === 'ArrowLeft' || e.key === 'ArrowUp' ? -1 : 0;
    if (!step || !options.length) return;
    e.preventDefault();
    onPick(options[(Math.max(at, 0) + step + options.length) % options.length].value);
  };
  return (
    <div role="radiogroup" aria-label={label} onKeyDown={onKey} className={cn('seg', small && 'seg-sm', fill && 'seg-fill')}>
      {options.map((o, i) => (
        <button key={String(o.value)} type="button" role="radio" aria-checked={i === at} aria-label={o.name} tabIndex={i === at || (at < 0 && i === 0) ? 0 : -1} onClick={() => onPick(o.value)}>
          {i === at ? <motion.span layoutId={`${id}-thumb`} className="seg-thumb" transition={{ type: 'spring', stiffness: 560, damping: 40, mass: 0.8 }} /> : null}
          <span className="seg-label">{o.label}</span>
        </button>
      ))}
    </div>
  );
}

/** A row that chooses between a few words, each named `<path>.<value>` in the dictionary. */
export function SegRow({
  path, title, desc, values, label, when,
}: { path: string; title: string; desc?: ReactNode; values: Array<string | number>; label?: string; when?: boolean }) {
  const t = useT();
  const value = useSetting<string | number>(path);
  return (
    <Row title={title} desc={desc} when={when}>
      <Segmented label={label ?? title} value={value} options={values.map((v) => ({ value: v, label: t(`${path}.${v}`) }))} onPick={(v) => change(path, v)} />
    </Row>
  );
}
