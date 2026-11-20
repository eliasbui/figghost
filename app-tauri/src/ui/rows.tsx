// The rows of a page of settings: Astryx's own parts, joined to the state of the app. A row names a setting by its path; what
// the control says is the value in the state, and what it does is `change(path, value)`.
import type { ReactNode } from 'react';
import { change } from '@/stores/app';
import { useSetting, useT } from '@/stores/hooks';
import { Segmented, Select, Slider, Switch } from './controls';
import type { Option } from './controls';
import { Row } from './layout';
import { useHeld } from './held';

export type { Option } from './controls';

/** The switch of a boolean setting, in a row. `when` false greys the row: what it sets means nothing while another is off. */
export function ToggleRow({ path, title, desc, when = true, label }: { path: string; title: ReactNode; desc?: ReactNode; when?: boolean; label?: string }) {
  return (
    <Row title={title} desc={desc} when={when}>
      <Switch path={path} label={label ?? (typeof title === 'string' ? title : path)} />
    </Row>
  );
}

/** The pop-up of fixed choices of a setting. Numbers stay numbers; a value that is not among the options is added, as it is,
 *  rather than snapped to one. */
export function SelectRow({ path, title, desc, options, when = true }: { path: string; title: string; desc?: ReactNode; options: Option[]; when?: boolean }) {
  const value = useSetting<string | number>(path);
  return (
    <Row title={title} desc={desc} when={when}>
      <Select value={value} options={options} label={title} onChange={(next) => change(path, next)} />
    </Row>
  );
}

/** A choice between a few words, each named `<path>.<value>` in the dictionary. */
export function SegRow({ path, title, desc, values, label, when = true }: { path: string; title: string; desc?: ReactNode; values: Array<string | number>; label?: string; when?: boolean }) {
  const t = useT();
  const value = useSetting<string | number>(path);
  return (
    <Row title={title} desc={desc} when={when}>
      <Segmented label={label ?? title} value={value} options={values.map((v) => ({ value: v, label: t(`${path}.${v}`) }))} onPick={(next) => change(path, next)} />
    </Row>
  );
}

/** A track with a thumb, and what it says beside it. */
export function SliderRow({
  path, title, desc, min, max, step, format, when = true, onInput,
}: { path: string; title: string; desc?: ReactNode; min: number; max: number; step: number; format: (value: number) => string; when?: boolean; onInput?: () => void }) {
  const stored = useSetting<number>(path);
  // While the thumb is held, what it shows is its own: a state the app sent a moment ago must not pull it back.
  const [held, hold] = useHeld<number>();
  return (
    <Row title={title} desc={desc} when={when}>
      <Slider
        value={held ?? stored ?? min}
        min={min}
        max={max}
        step={step}
        label={title}
        format={format}
        onChange={(next) => {
          hold(next);
          change(path, next);
          onInput?.();
        }}
        onRelease={() => hold(null)}
      />
    </Row>
  );
}
