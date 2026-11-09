import { clsx } from 'clsx';
import type { ClassValue } from 'clsx';
import { twMerge } from 'tailwind-merge';

/** Class names joined, and where two utilities say the same thing the later one wins (`p-2 p-4` is `p-4`). */
export function cn(...names: ClassValue[]): string {
  return twMerge(clsx(names));
}
