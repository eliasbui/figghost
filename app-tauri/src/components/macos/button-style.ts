import { tv } from 'tailwind-variants';

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
