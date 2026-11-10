import { cn } from '@/lib/cn';

/** A symbol the system drew (a PNG, as a data URL), painted in the colour of the text round it. */
export function Glyph({ src, className }: { src?: string | null; className?: string }) {
  const mask = src ? `url("${src}") center / contain no-repeat` : undefined;
  return <span aria-hidden="true" className={cn('inline-block size-[15px] shrink-0 bg-current', !src && 'invisible', className)} style={mask ? { WebkitMask: mask, mask } : undefined} />;
}
