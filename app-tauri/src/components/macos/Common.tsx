import { useEffect, useRef, useState } from 'react';
import { change, send } from '@/stores/app';
import { useT } from '@/stores/hooks';
import type { ReactNode } from 'react';
import { ActionButton, ActionIcon, IconButton, Select } from './Controls';
import { HotkeyButton } from './Hotkey';
import { Pill, Row } from './Layout';

// Rows that most features have, named by the prefix their words share in the dictionary (`smart.access.title`).

/** Whether the app has the Accessibility access it needs, said in a word. */
export function AccessPill({ granted }: { granted: boolean }) {
  const t = useT();
  return <Pill good={granted}>{t(granted ? 'chip.access.ok' : 'chip.access.no')}</Pill>;
}

/** The Accessibility access of a feature: whether it is granted, and a way to the place that grants it. */
export function AccessRow({ ns, granted }: { ns: string; granted: boolean }) {
  const t = useT();
  return (
    <Row title={t(`${ns}.access.title`)} desc={t(`${ns}.access.desc`)}>
      <AccessPill granted={granted} />
      <IconButton icon="open" label={t(`${ns}.access.open`)} variant={granted ? 'plain' : 'glass'} onClick={() => send({ t: 'action', name: 'open_accessibility' })} />
    </Row>
  );
}

/** A permission: its name, whether it is given (nothing is said while that is not known), and a way to the place that gives it. */
export function PermissionRow({ title, given, detail, action, label, when }: { title: ReactNode; given: boolean | null | undefined; detail?: ReactNode; action?: string; label?: string; when?: boolean }) {
  const t = useT();
  return (
    <Row title={title} desc={detail} when={when}>
      {given === null || given === undefined ? null : <Pill good={given}>{t(given ? 'access.granted' : 'access.missing')}</Pill>}
      {action ? <ActionIcon name={action} icon="open" label={label ?? ''} /> : null}
    </Row>
  );
}

/** The shortcut of a feature, which works from any app. */
export function ShortcutRow({ ns, target = ns, when, allowClear }: { ns: string; target?: string; when?: boolean; allowClear?: boolean }) {
  const t = useT();
  return (
    <Row title={t(`${ns}.shortcut.title`)} desc={t(`${ns}.shortcut.desc`)} when={when}>
      <HotkeyButton target={target} label={t(`${ns}.shortcut.title`)} allowClear={allowClear} />
    </Row>
  );
}

/** A button that shows the feature at work (`<ns>.try.*`, the action `<ns>_try`). */
export function TryRow({ ns, when }: { ns: string; when?: boolean }) {
  const t = useT();
  return (
    <Row title={t(`${ns}.try.title`)} desc={t(`${ns}.try.desc`)} when={when}>
      <ActionButton name={`${ns}_try`} icon="play">
        {t(`${ns}.try.button`)}
      </ActionButton>
    </Row>
  );
}

/** A pop-up of languages: Automatic and the ones the app knows, by their code. */
export function LanguageSelect({ path, languages, value, label, autoLabel, disabled }: { path: string; languages: Array<{ code: string; name: string }>; value: string; label: string; autoLabel: string; disabled?: boolean }) {
  return (
    <Select
      label={label}
      value={value}
      disabled={disabled}
      options={[{ value: 'auto', label: autoLabel }, ...languages.map((l) => ({ value: l.code, label: l.name }))]}
      onChange={(v) => change(path, v)}
    />
  );
}

/** A line of words that are a list: they are split at spaces, commas and semicolons when the field is left. */
export function ListField({ values, onCommit, label, placeholder }: { values: string[]; onCommit: (values: string[]) => void; label: string; placeholder?: string }) {
  const shown = values.join(' ');
  const [text, setText] = useState(shown);
  const editing = useRef(false);
  // What the app says replaces the text, unless the person is in the middle of typing.
  useEffect(() => {
    if (!editing.current) setText(shown);
  }, [shown]);
  const commit = () => {
    editing.current = false;
    const parsed = text.split(/[\s,;]+/).filter(Boolean);
    if (parsed.join(' ') !== shown) onCommit(parsed);
    else setText(shown);
  };
  return (
    <input
      type="text"
      className="text-field"
      aria-label={label}
      placeholder={placeholder}
      spellCheck={false}
      autoComplete="off"
      autoCapitalize="off"
      value={text}
      onFocus={() => {
        editing.current = true;
      }}
      onChange={(e) => setText(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === 'Enter') e.currentTarget.blur();
      }}
    />
  );
}
