import { useEffect, useState } from 'react';
import type { ButtonHTMLAttributes, ReactNode, Ref } from 'react';
import { tv } from 'tailwind-variants';
import type { VariantProps } from 'tailwind-variants';
import { cn } from '@/lib/cn';
import { send } from '@/stores/app';
import { Icon } from './Icon';
import { Tip } from './Overlay';

// -- the button, in all its kinds

/** One recipe for every button: a kind (`variant`), a size and a shape. Glass is the plain kind; the accent one is for the
 *  single thing a window wants done; the rest are quieter. */
export const button = tv({
  base: [
    'relative inline-flex shrink-0 items-center justify-center gap-1.5 whitespace-nowrap font-medium tracking-[-0.005em] select-none',
    'transition-[transform,background-color,box-shadow,color,opacity] duration-200 ease-spring',
    'disabled:pointer-events-none disabled:opacity-40 aria-disabled:pointer-events-none aria-disabled:opacity-40',
    '[&_.glyph]:size-[1.2em] [&_.glyph]:shrink-0',
  ],
  variants: {
    variant: {
      /** Glass: the everyday button. */
      glass: 'gl text-fg',
      /** The accent as glass: the one thing the window wants done. */
      prominent: 'gl gl-tint',
      /** The accent as a wash, for a second choice beside a prominent one. */
      tinted: 'bg-accent-soft text-accent shadow-[inset_0_0_0_0.5px_color-mix(in_srgb,var(--accent)_32%,transparent)] hover:bg-[color-mix(in_srgb,var(--accent)_26%,transparent)] active:scale-[0.965]',
      /** No surface until it is touched: for a symbol in a row or a bar. */
      plain: 'text-fg-2 hover:bg-[var(--fill)] hover:text-fg active:scale-[0.94] active:bg-[color-mix(in_srgb,var(--fg)_12%,transparent)]',
      /** A line round it. */
      outline: 'text-fg shadow-[inset_0_0_0_1px_var(--hair-2)] hover:bg-[var(--fill)] active:scale-[0.965]',
      /** What undoes something: red as glass. */
      danger: 'gl text-bad',
      /** Red, filled: the one that cannot be taken back. */
      destructive: 'gl text-white bg-[linear-gradient(180deg,color-mix(in_oklab,var(--bad)_82%,white),var(--bad)_50%,color-mix(in_oklab,var(--bad)_88%,black))] shadow-[inset_0_0_0_0.5px_rgb(0_0_0/0.3),inset_0_1px_0_rgb(255_255_255/0.45),0_1px_2px_rgb(0_0_0/0.2)]',
      /** Words in the accent, no surface. */
      link: 'h-auto! rounded-md! px-0.5! text-accent hover:underline underline-offset-2 active:opacity-70',
    },
    size: {
      xs: 'h-5 min-w-5 px-2 text-[11px]',
      sm: 'h-6 min-w-6 px-2.5 text-[12px]',
      md: 'h-7 min-w-7 px-3.5 text-[13px]',
      lg: 'h-[34px] min-w-[34px] px-5 text-[14px]',
    },
    shape: {
      capsule: 'rounded-full',
      rounded: 'rounded-[10px]',
    },
    /** Only a symbol: as wide as it is tall. */
    only: {
      true: 'aspect-square px-0',
    },
    block: {
      true: 'w-full',
    },
  },
  defaultVariants: { variant: 'glass', size: 'md', shape: 'capsule' },
});

type Recipe = VariantProps<typeof button>;

export interface ButtonProps extends Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'type'> {
  variant?: Recipe['variant'];
  size?: Recipe['size'];
  shape?: Recipe['shape'];
  block?: boolean;
  /** The name of a picture drawn before the words. */
  icon?: string;
  /** A picture drawn after the words. */
  iconEnd?: string;
  /** Busy: a spinner takes the place of the picture and the button cannot be pressed. */
  loading?: boolean;
  /** The older way to say `variant="prominent"`. */
  primary?: boolean;
  /** The older way to say `variant="danger"`, or `destructive` with `primary`. */
  danger?: boolean;
  /** The older way to say a button for a dense list: a size down. */
  small?: boolean;
  ref?: Ref<HTMLButtonElement>;
}

export function Button({ variant, size, shape, block, icon, iconEnd, loading, primary, danger, small, className, children, disabled, ref, ...rest }: ButtonProps) {
  const kind = variant ?? (danger ? (primary ? 'destructive' : 'danger') : primary ? 'prominent' : 'glass');
  const only = !children && !!(icon || iconEnd) ? true : undefined;
  return (
    <button ref={ref} type="button" className={cn(button({ variant: kind, size: size ?? (small && only ? 'sm' : 'md'), shape, only, block }), className)} disabled={disabled || loading} aria-busy={loading || undefined} {...rest}>
      {loading ? <Spinner /> : icon ? <Icon name={icon} /> : null}
      {children}
      {iconEnd ? <Icon name={iconEnd} /> : null}
    </button>
  );
}

function Spinner() {
  return <span aria-hidden="true" className="size-[1em] animate-spin rounded-full border-[1.5px] border-current border-t-transparent" />;
}

/** A button that asks the app to do something: `name` is the action it is told, as in `{ t: 'action', name }`. */
export function ActionButton({ name, ...rest }: ButtonProps & { name: string }) {
  return <Button onClick={() => send({ t: 'action', name })} {...rest} />;
}

/** A button with only a picture that asks the app to do something; the words are its tip. */
export function ActionIcon({ name, icon, label, variant = 'glass', ...rest }: Omit<ButtonProps, 'children' | 'icon' | 'iconEnd' | 'aria-label' | 'title'> & { name: string; icon: string; label: string }) {
  return <IconButton icon={icon} label={label} variant={variant} onClick={() => send({ t: 'action', name })} {...rest} />;
}

/** A button that asks twice before it does something that cannot be undone, so that one stray click does not. */
export function SureButton({ label, sure, onConfirm, size = 'md', icon }: { label: string; sure: string; onConfirm: () => void; size?: Recipe['size']; icon?: string }) {
  const [armed, setArmed] = useState(false);
  useEffect(() => {
    if (!armed) return;
    const timer = setTimeout(() => setArmed(false), 4000);
    return () => clearTimeout(timer);
  }, [armed]);
  return (
    <Button
      size={size}
      variant={armed ? 'destructive' : 'danger'}
      icon={armed ? undefined : icon}
      onClick={() => {
        if (!armed) {
          setArmed(true);
          return;
        }
        setArmed(false);
        onConfirm();
      }}
    >
      {armed ? sure : label}
    </Button>
  );
}

/** A button with a picture and no words. The label is for the reader of the screen, and comes up as a tip. */
export function IconButton({
  icon, label, variant = 'plain', size = 'md', shape = 'capsule', tip = true, className, ref, ...rest
}: Omit<ButtonProps, 'children' | 'icon' | 'iconEnd' | 'aria-label' | 'title'> & { icon: string; label: string; tip?: boolean }) {
  const control = (
    <Button ref={ref} variant={variant} size={size} shape={shape} icon={icon} aria-label={label} className={className} {...rest} />
  );
  return tip ? <Tip text={label}>{control}</Tip> : control;
}

/** Buttons that belong together, in one capsule of glass with a line between them. */
export function ButtonGroup({ children, className, label }: { children: ReactNode; className?: string; label?: string }) {
  return (
    <div role="group" aria-label={label} className={cn('gl inline-flex items-center rounded-full p-[3px] [&>button]:shadow-none', '[&>*+*]:relative [&>*+*]:before:absolute [&>*+*]:before:-left-[1px] [&>*+*]:before:top-1.5 [&>*+*]:before:bottom-1.5 [&>*+*]:before:w-px [&>*+*]:before:bg-hair', className)}>
      {children}
    </div>
  );
}

/** A button that stays pressed: on or off. */
export function ToggleButton({ pressed, onPressedChange, children, icon, size, className, ...rest }: Omit<ButtonProps, 'variant' | 'onClick'> & { pressed: boolean; onPressedChange: (pressed: boolean) => void }) {
  return (
    <Button variant={pressed ? 'tinted' : 'glass'} size={size} icon={icon} aria-pressed={pressed} className={className} onClick={() => onPressedChange(!pressed)} {...rest}>
      {children}
    </Button>
  );
}
