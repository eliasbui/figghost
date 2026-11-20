// The controls of a page, as the pages ask for them: Astryx's own parts with the words the pages already use (a name for a
// picture, `primary`, `small`, `disabled`). Nothing here draws anything of its own.
import { useEffect, useState } from 'react';
import type { ReactNode } from 'react';
import { ButtonGroup as AstryxButtonGroup } from '@astryxdesign/core/ButtonGroup';
import { CodeBlock as AstryxCodeBlock } from '@astryxdesign/core/CodeBlock';
import { DropdownMenu } from '@astryxdesign/core/DropdownMenu';
import { HStack } from '@astryxdesign/core/HStack';
import { InfoTip as AstryxInfoTip } from '@astryxdesign/core/InfoTip';
import { Kbd } from '@astryxdesign/core/Kbd';
import { SegmentedControl, SegmentedControlItem } from '@astryxdesign/core/SegmentedControl';
import { Selector } from '@astryxdesign/core/Selector';
import { Slider as AstryxSlider } from '@astryxdesign/core/Slider';
import { Switch as AstryxSwitch } from '@astryxdesign/core/Switch';
import { Text } from '@astryxdesign/core/Text';
import { ToggleButton as AstryxToggleButton } from '@astryxdesign/core/ToggleButton';
import { keyLabel, shortcutParts } from '@/lib/shortcuts';
import { change, send } from '@/stores/app';
import { useSetting } from '@/stores/hooks';
import { glyph, iconType } from './Icon';

// -- buttons

import { Button, IconButton, SIZE, wordsOf } from './button';
import type { ButtonProps } from './button';

export { Button, IconButton };
export type { ButtonProps };

/** A button that asks the app to do something: `name` is the action it is told, as in `{ t: 'action', name }`. */
export function ActionButton({ name, ...rest }: ButtonProps & { name: string }) {
  return <Button onClick={() => send({ t: 'action', name })} {...rest} />;
}

/** A button with only a picture that asks the app to do something; the words are its tip. */
export function ActionIcon({ name, icon, label, variant = 'secondary', ...rest }: Omit<ButtonProps, 'children' | 'icon' | 'iconEnd' | 'label' | 'title'> & { name: string; icon: string; label: string }) {
  return <IconButton icon={icon} label={label} variant={variant} onClick={() => send({ t: 'action', name })} {...rest} />;
}

/** A button that asks twice before it does something that cannot be undone, so that one stray click does not. */
export function SureButton({ label, sure, onConfirm, size, icon }: { label: string; sure: string; onConfirm: () => void; size?: ButtonProps['size']; icon?: string }) {
  const [armed, setArmed] = useState(false);
  useEffect(() => {
    if (!armed) return;
    const timer = setTimeout(() => setArmed(false), 4000);
    return () => clearTimeout(timer);
  }, [armed]);
  return (
    <Button
      size={size}
      variant={armed ? 'destructive' : 'secondary'}
      icon={armed ? undefined : icon}
      label={armed ? sure : label}
      onClick={() => {
        if (!armed) return setArmed(true);
        setArmed(false);
        onConfirm();
      }}
    />
  );
}

/** Buttons that belong together, in one group. */
export function ButtonGroup({ children, label }: { children: ReactNode; className?: string; label?: string }) {
  return <AstryxButtonGroup label={label ?? ''}>{children}</AstryxButtonGroup>;
}

/** A button that stays pressed: on or off. */
export function ToggleButton({ pressed, onPressedChange, children, label, icon, size, disabled }: { pressed: boolean; onPressedChange: (pressed: boolean) => void; children?: ReactNode; label?: string; icon?: string; size?: ButtonProps['size']; disabled?: boolean }) {
  return <AstryxToggleButton label={label ?? wordsOf(children)} icon={glyph(icon)} size={SIZE[size ?? 'sm']} isPressed={pressed} isDisabled={disabled} onPressedChange={onPressedChange} />;
}

// -- a button that opens a short list

export interface MenuEntry {
  id: string;
  label: string;
  icon?: string;
  danger?: boolean;
  disabled?: boolean;
  onAction?: () => void;
  /** A line above this entry. */
  divider?: boolean;
}

/** A button that opens a menu. */
export function MenuButton({
  label, icon, iconEnd, variant, size, entries, placement = 'bottom end',
}: { label: string; icon?: string; iconEnd?: string; variant?: ButtonProps['variant']; size?: ButtonProps['size']; entries: MenuEntry[]; placement?: string }) {
  return (
    <DropdownMenu
      button={{ label, variant: variant ?? 'secondary', size: SIZE[size ?? 'sm'], icon: glyph(icon) }}
      hasChevron={!iconEnd}
      placement={placement.startsWith('top') ? 'above' : 'below'}
      alignment={placement.endsWith('end') ? 'end' : 'start'}
      items={entries.flatMap((e) => [
        ...(e.divider ? [{ type: 'divider' as const }] : []),
        { id: e.id, label: e.label, icon: iconType(e.icon ?? ''), isDisabled: e.disabled, variant: e.danger ? ('destructive' as const) : ('default' as const), onClick: e.onAction },
      ])}
    />
  );
}

// -- switches, pop-ups, choices, tracks

interface SwitchButtonProps {
  checked: boolean;
  onToggle: () => void;
  label?: string;
  disabled?: boolean;
  /** The setting it stands for, for the page's own search. */
  path?: string;
}

/** A switch that is on or off. */
export function SwitchButton({ checked, onToggle, label, disabled }: SwitchButtonProps) {
  return <AstryxSwitch label={label ?? ''} isLabelHidden size="sm" value={checked} isDisabled={disabled} onChange={onToggle} />;
}

/** The switch of a boolean setting. */
export function Switch({ path, label }: { path: string; label?: string }) {
  const on = !!useSetting<boolean>(path);
  return <SwitchButton checked={on} onToggle={() => change(path, !on)} label={label ?? path} />;
}

export interface Option {
  value: string | number;
  label: string;
}

/** The pop-up of fixed choices. Numbers stay numbers; a value that is not among the options is added, as it is, rather than
 *  snapped to one. */
export function Select({
  value, options, onChange, label, disabled,
}: { value: string | number | undefined; options: Option[]; onChange: (value: string | number) => void; label?: string; disabled?: boolean; small?: boolean }) {
  const none = value === undefined || value === null;
  const list = none || options.some((o) => String(o.value) === String(value)) ? options : [...options, { value, label: String(value) }];
  return (
    <Selector
      label={label ?? ''}
      isLabelHidden
      size="sm"
      variant="secondary"
      value={none ? undefined : String(value)}
      options={list.map((o) => ({ value: String(o.value), label: o.label }))}
      isDisabled={disabled}
      onChange={(next) => onChange(typeof value === 'number' ? +next : next)}
    />
  );
}

export interface Choice<V extends string | number = string> {
  value: V;
  label: ReactNode;
  /** The words for the reader of the screen when the label is only a picture. */
  name?: string;
}

/** A well with a thumb that moves to the choice. A value that is none of the choices chooses none. */
export function Segmented<V extends string | number>({
  options, value, onPick, label, fill, disabled,
}: { options: Choice<V>[]; value: V | undefined; onPick: (value: V) => void; small?: boolean; label?: string; fill?: boolean; disabled?: boolean }) {
  return (
    <SegmentedControl
      label={label ?? ''}
      size="sm"
      layout={fill ? 'fill' : 'hug'}
      value={value === undefined ? '' : String(value)}
      isDisabled={disabled}
      onChange={(next) => {
        const hit = options.find((o) => String(o.value) === next);
        if (hit) onPick(hit.value);
      }}
    >
      {options.map((o) =>
        typeof o.label === 'string' || typeof o.label === 'number' ? (
          <SegmentedControlItem key={String(o.value)} value={String(o.value)} label={String(o.label)} />
        ) : (
          <SegmentedControlItem key={String(o.value)} value={String(o.value)} label={o.name ?? String(o.value)} icon={o.label} isLabelHidden />
        ),
      )}
    </SegmentedControl>
  );
}

/** A track with a thumb, and what it says beside it. */
export function Slider({
  value, min, max, step, onChange, label, format, onRelease, disabled,
}: {
  value: number;
  min: number;
  max: number;
  step: number;
  onChange: (value: number) => void;
  label: string;
  format?: (value: number) => string;
  onRelease?: () => void;
  disabled?: boolean;
}) {
  return (
    <AstryxSlider
      label={label}
      isLabelHidden
      value={value}
      min={min}
      max={max}
      step={step}
      width={220}
      formatValue={format}
      valueDisplay={format ? 'text' : 'none'}
      isDisabled={disabled}
      onChange={onChange}
      onChangeEnd={onRelease}
    />
  );
}

// -- words about a control

/** The "i" beside a name: what it would say in words, kept until it is asked for. With `text` it is a short line that
 *  carries the "i". */
export function InfoTip({ children, label, text }: { children: ReactNode; label?: string; text?: ReactNode; bad?: boolean }) {
  const tip = <AstryxInfoTip content={children} label={label} />;
  return text ? (
    <HStack gap={1} align="center">
      <Text type="supporting">{text}</Text>
      {tip}
    </HStack>
  ) : (
    tip
  );
}

/** A shortcut as the keys of a keyboard. */
export function Keycaps({ shortcut }: { shortcut: string }) {
  return <Kbd keys={shortcutParts(shortcut).map(keyLabel).join('+')} />;
}

export function CodeBlock({ children, label }: { children: string; label?: string }) {
  return <AstryxCodeBlock code={children} title={label} hasCopyButton={false} />;
}
