import type { HTMLAttributes, MouseEvent, ReactNode, Ref } from 'react';
import { Banner } from '@astryxdesign/core/Banner';
import { Card as AstryxCard } from '@astryxdesign/core/Card';
import { NumberInput } from '@astryxdesign/core/NumberInput';
import { Selector } from '@astryxdesign/core/Selector';
import { Switch } from '@astryxdesign/core/Switch';
import { cn } from '@/lib/cn';
import { Btn } from '../ui';

export { Btn };

/** The title of a page and the line under it. */
export function Header({ title, sub }: { title: string; sub: string }) {
  return (
    <>
      <h1 className="mb-1 text-[22px] font-semibold">{title}</h1>
      <p className="mb-[18px] max-w-[640px] text-fg-2">{sub}</p>
    </>
  );
}

export function Section({ children }: { children?: ReactNode }) {
  return <h2 className="mt-[22px] mb-2 text-[13px] font-semibold text-fg-2">{children}</h2>;
}

export function Card({ children, className, ...rest }: { children: ReactNode; className?: string } & HTMLAttributes<HTMLDivElement>) {
  return (
    <AstryxCard padding={0} className={className} {...rest}>
      {children}
    </AstryxCard>
  );
}

/** A line of a card: what it is on the left (and a hint under it), what to do on the right. */
export function Row({ children, className, ref, ...rest }: { children: ReactNode; className?: string; ref?: Ref<HTMLDivElement> } & HTMLAttributes<HTMLDivElement>) {
  return (
    <div ref={ref} className={cn('flex min-h-11 items-center gap-3 px-3.5 py-2.5 not-first:border-t-[0.5px] not-first:border-hair', className)} {...rest}>
      {children}
    </div>
  );
}

export const Grow = ({ children, className }: { children: ReactNode; className?: string }) => <div className={cn('min-w-0 flex-1', className)}>{children}</div>;
export const Hint = ({ children, className }: { children: ReactNode; className?: string }) => <div className={cn('text-xs text-fg-2', className)}>{children}</div>;

/** A notice: something the person should know before they start. */
export function Notice({ children }: { children: ReactNode }) {
  return (
    <div className="mb-4">
      <Banner status="warning" title={children} collapsible={false} />
    </div>
  );
}

export function Toggle({ checked, onChange, label }: { checked: boolean; onChange: (on: boolean) => void; label: string }) {
  return <Switch label={label} isLabelHidden size="sm" value={checked} onChange={onChange} />;
}

export function Select({ value, options, onChange, label }: { value: string; options: { id: string; text: string }[]; onChange: (id: string) => void; label: string }) {
  return <Selector label={label} isLabelHidden size="sm" variant="secondary" value={value} options={options.map((o) => ({ value: o.id, label: o.text }))} onChange={onChange} />;
}

/** A line of text. */
export { Field as Input } from '@/ui/field';

/** A number, which is sent when the person is through with it. `key` the field by the value the app holds, so that what the app
 *  says replaces what was typed. */
export function NumberField({ value, min, max, onChange, label, width = 100 }: { value: number; min: number; max: number; onChange: (value: number) => void; label: string; width?: number }) {
  return <NumberInput label={label} isLabelHidden size="sm" width={width} min={min} max={max} value={value} onChange={onChange} />;
}

/** A small button inside a row of tools. */
export function Tool({ children, ...rest }: { children: ReactNode; className?: string; title?: string; onClick?: (event: MouseEvent) => void; 'aria-label'?: string; 'aria-pressed'?: boolean }) {
  return (
    <Btn variant="ghost" small {...rest}>
      {children}
    </Btn>
  );
}

export const Mono = ({ children, className }: { children: ReactNode; className?: string }) => (
  <div className={cn('rounded-md bg-[var(--field)] px-2 py-1.5 font-mono text-xs break-all shadow-[inset_0_0_0_0.5px_var(--field-edge)] select-all', className)}>{children}</div>
);
