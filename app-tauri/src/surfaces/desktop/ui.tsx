import type { ButtonHTMLAttributes, HTMLAttributes, InputHTMLAttributes, ReactNode, Ref, SelectHTMLAttributes } from 'react';
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
    <div className={cn('overflow-hidden rounded-[var(--r-card)] bg-card shadow-[inset_0_0_0_0.5px_var(--card-edge)]', className)} {...rest}>
      {children}
    </div>
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
  return <div className="mb-4 rounded-[var(--r-field)] bg-[color-mix(in_srgb,var(--warn)_12%,transparent)] px-3.5 py-2.5 shadow-[inset_0_0_0_1px_var(--warn)]">{children}</div>;
}

export function Toggle({ checked, onChange, label }: { checked: boolean; onChange: (on: boolean) => void; label: string }) {
  return <button type="button" role="switch" aria-checked={checked} aria-label={label} className="switch" onClick={() => onChange(!checked)} />;
}

const field = 'h-7 rounded-[var(--r-field)] bg-[var(--field)] px-2.5 shadow-[inset_0_0_0_0.5px_var(--field-edge)] focus:bg-card focus:shadow-[inset_0_0_0_0.5px_var(--accent),0_0_0_3px_var(--accent-ring)]';

export function Select({ value, options, onChange, label }: { value: string; options: { id: string; text: string }[]; onChange: (id: string) => void; label: string } & Omit<SelectHTMLAttributes<HTMLSelectElement>, 'value' | 'onChange'>) {
  return (
    <select aria-label={label} className={cn(field, 'cursor-default')} value={value} onChange={(event) => onChange(event.target.value)}>
      {options.map((o) => (
        <option key={o.id} value={o.id}>
          {o.text}
        </option>
      ))}
    </select>
  );
}

export function Input({ className, ...rest }: InputHTMLAttributes<HTMLInputElement> & { ref?: Ref<HTMLInputElement> }) {
  return <input className={cn(field, className)} {...rest} />;
}

/** A small button inside a row of tools. */
export function Tool({ className, ...rest }: ButtonHTMLAttributes<HTMLButtonElement>) {
  return <button type="button" className={cn('rounded-[5px] px-1.5 py-1 text-xs text-fg-2 hover:bg-[var(--fill)] hover:text-fg', className)} {...rest} />;
}

export const Mono = ({ children, className }: { children: ReactNode; className?: string }) => (
  <div className={cn('rounded-md bg-[var(--field)] px-2 py-1.5 font-mono text-xs break-all shadow-[inset_0_0_0_0.5px_var(--field-edge)] select-all', className)}>{children}</div>
);
