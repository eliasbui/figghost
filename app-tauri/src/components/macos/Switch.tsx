import type { ReactNode } from 'react';
import { change } from '@/stores/app';
import { useSetting } from '@/stores/hooks';
import { Row } from './Layout';

interface SwitchButtonProps {
  checked: boolean;
  onToggle: () => void;
  label?: string;
  disabled?: boolean;
  /** The setting it stands for, for the page's own search. */
  path?: string;
}

/** A capsule that is on or off, with a knob of glass that stretches while it is held. */
export function SwitchButton({ checked, onToggle, label, disabled, path }: SwitchButtonProps) {
  return <button type="button" className="switch" role="switch" aria-checked={checked} aria-label={label} data-path={path} disabled={disabled} onClick={onToggle} />;
}

/** The switch of a boolean setting. */
export function Switch({ path, label }: { path: string; label?: string }) {
  const on = !!useSetting<boolean>(path);
  return <SwitchButton checked={on} onToggle={() => change(path, !on)} label={label} path={path} />;
}

export function ToggleRow({ path, title, desc, when, label }: { path: string; title: ReactNode; desc?: ReactNode; when?: boolean; label?: string }) {
  return (
    <Row title={title} desc={desc} when={when}>
      <Switch path={path} label={label ?? (typeof title === 'string' ? title : undefined)} />
    </Row>
  );
}
