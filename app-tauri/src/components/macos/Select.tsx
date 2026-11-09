import type { Key, ReactNode } from 'react';
import { ListBox, ListBoxItem, Popover, Select as AriaSelect, SelectValue, Button as AriaButton } from 'react-aria-components';
import { cn } from '@/lib/cn';
import { change } from '@/stores/app';
import { useSetting } from '@/stores/hooks';
import { Icon } from './Icon';
import { Row } from './Layout';

export interface Option {
  value: string | number;
  label: string;
}

const POPOVER = cn(
  'glass-pop min-w-[var(--trigger-width)] max-h-[320px] overflow-auto rounded-[var(--r-pop)] p-1.5 text-[13px] outline-none',
  'data-[entering]:animate-[pop-in_180ms_var(--ease-out)] data-[exiting]:animate-[pop-out_110ms_ease-in_forwards]',
);

/** The pop-up of fixed choices. Numbers stay numbers; a value that is not among the options is added, as it is, rather than
 *  snapped to one. */
export function Select({
  value, options, onChange, label, disabled, small,
}: { value: string | number; options: Option[]; onChange: (value: string | number) => void; label?: string; disabled?: boolean; small?: boolean }) {
  const has = options.some((o) => String(o.value) === String(value));
  const list = has || value === undefined || value === null ? options : [...options, { value, label: String(value) }];
  const pick = (key: Key | null) => {
    if (key === null) return;
    onChange(typeof value === 'number' ? +String(key) : String(key));
  };
  return (
    <AriaSelect selectedKey={value === undefined || value === null ? null : String(value)} onSelectionChange={pick} isDisabled={disabled} aria-label={label} className="select-root" data-small={small ? '' : undefined}>
      <AriaButton className="select-trigger gl">
        <SelectValue className="truncate" />
        <Icon name="caret.updown" className="select-caret" />
      </AriaButton>
      <Popover className={POPOVER} offset={6}>
        <ListBox className="outline-none">
          {list.map((o) => (
            <ListBoxItem key={String(o.value)} id={String(o.value)} textValue={o.label} className="select-item">
              {({ isSelected }: { isSelected: boolean }) => (
                <>
                  <span className="select-check">{isSelected ? <Icon name="check" /> : null}</span>
                  <span className="truncate">{o.label}</span>
                </>
              )}
            </ListBoxItem>
          ))}
        </ListBox>
      </Popover>
    </AriaSelect>
  );
}

export function SelectRow({ path, title, desc, options, when }: { path: string; title: string; desc?: ReactNode; options: Option[]; when?: boolean }) {
  const value = useSetting<string | number>(path);
  return (
    <Row title={title} desc={desc} when={when}>
      <Select value={value} options={options} label={title} onChange={(v) => change(path, v)} />
    </Row>
  );
}
