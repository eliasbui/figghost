import type { ButtonHTMLAttributes, ReactNode } from 'react';
import type { VariantProps } from 'tailwind-variants';
import { button } from '@/components/macos/button-style';
import { cn } from '@/lib/cn';

type Recipe = VariantProps<typeof button>;

/** The kit's button, for a surface that has no store to lean on. */
export function Btn({ variant, size, shape, className, ...rest }: Omit<ButtonHTMLAttributes<HTMLButtonElement>, 'type'> & Pick<Recipe, 'variant' | 'size' | 'shape'>) {
  return <button type="button" className={cn(button({ variant, size, shape }), className)} {...rest} />;
}

/** A state said in a word and a dot (the kit's `.pill`). */
export function Pill({ tone, children }: { tone: 'good' | 'warn' | 'bad' | 'off'; children: ReactNode }) {
  return (
    <span className="pill" data-tone={tone}>
      <span className="dot" />
      <span>{children}</span>
    </span>
  );
}
