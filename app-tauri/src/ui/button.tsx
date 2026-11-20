// The button, as the pages ask for it: Astryx's own with the words the pages already use (a name for a picture, `primary`,
// `small`, `disabled`). It leans on no store, so a surface that has none can use it.
import type { CSSProperties, MouseEvent, ReactNode, Ref } from 'react';
import { Button as AstryxButton } from '@astryxdesign/core/Button';
import { IconButton as AstryxIconButton } from '@astryxdesign/core/IconButton';
import { glyph } from './Icon';

export type Kind = 'primary' | 'secondary' | 'ghost' | 'destructive';

export const SIZE = { xs: 'sm', sm: 'sm', md: 'md', lg: 'lg' } as const;

export interface ButtonProps {
  /** The words of the button. */
  children?: ReactNode;
  /** The words, when they are not the children. */
  label?: string;
  variant?: Kind;
  size?: keyof typeof SIZE;
  block?: boolean;
  /** The name of a picture drawn before the words. */
  icon?: string;
  /** A picture drawn after the words. */
  iconEnd?: string;
  loading?: boolean;
  disabled?: boolean;
  /** The older way to say `variant="primary"`. */
  primary?: boolean;
  /** The older way to say `variant="destructive"`. */
  danger?: boolean;
  small?: boolean;
  title?: string;
  className?: string;
  /** Kept in its place but not drawn, or drawn another way (`visibility`). */
  hidden?: boolean;
  style?: CSSProperties;
  /** The older words for the shape of a glass button; an Astryx button has one shape. */
  shape?: 'capsule' | 'rounded';
  onClick?: (event: MouseEvent) => void;
  ref?: Ref<HTMLButtonElement>;
  'aria-label'?: string;
  'aria-expanded'?: boolean;
}

/** The words of a node, for the name a button is known by. */
export function wordsOf(node: ReactNode): string {
  if (typeof node === 'string' || typeof node === 'number') return String(node);
  if (Array.isArray(node)) return node.map(wordsOf).join('');
  return '';
}

export function Button({ variant, size, block, icon, iconEnd, loading, disabled, primary, danger, small, label, children, title, hidden, shape: _shape, ...rest }: ButtonProps) {
  const kind = variant ?? (danger ? 'destructive' : primary ? 'primary' : 'secondary');
  const words = label ?? wordsOf(children);
  if (hidden) return null;
  return (
    <AstryxButton
      {...rest}
      label={words || title || rest['aria-label'] || ''}
      variant={kind}
      size={SIZE[size ?? (small || !size ? 'sm' : 'md')]}
      width={block ? '100%' : undefined}
      icon={glyph(icon)}
      endContent={glyph(iconEnd)}
      isLoading={loading}
      isDisabled={disabled}
      tooltip={title}
    >
      {children && typeof children !== 'string' && typeof children !== 'number' && !wordsOf(children) ? children : undefined}
    </AstryxButton>
  );
}

/** A button with only a picture; the words are its tip and its name for the reader of the screen. */
export function IconButton({
  icon, label, variant = 'ghost', size = 'sm', tip = true, disabled, className, onClick, ref, ...rest
}: Omit<ButtonProps, 'children' | 'icon' | 'iconEnd' | 'label' | 'title'> & { icon: string; label: string; tip?: boolean }) {
  return (
    <AstryxIconButton
      {...rest}
      ref={ref}
      label={label}
      icon={glyph(icon)}
      variant={variant}
      size={SIZE[size]}
      isDisabled={disabled}
      tooltip={tip ? label : undefined}
      className={className}
      onClick={onClick}
    />
  );
}
