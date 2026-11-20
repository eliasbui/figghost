import type { ReactNode } from 'react';
import { Badge } from '@astryxdesign/core/Badge';
import { Button } from '@/ui/button';

/** The button of the kit's words, for a surface that has no store to lean on. */
export { Button as Btn };

const TONE = { good: 'success', warn: 'warning', bad: 'error', off: 'neutral' } as const;

/** A state said in a word. */
export function Pill({ tone, children }: { tone: 'good' | 'warn' | 'bad' | 'off'; children: ReactNode }) {
  return <Badge variant={TONE[tone]} label={children} />;
}
