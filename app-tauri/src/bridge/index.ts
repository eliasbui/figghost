import type { InboundHandler, Outbound } from './messages';

/** The way the page talks to the app: one message at a time out, and the messages of the app in. */
export interface Bridge {
  readonly kind: 'tauri' | 'fixture';
  send(message: Outbound): void;
  /** Resolves once the listening has started, so the page can say it is ready without missing the answer. */
  subscribe(handler: InboundHandler): Promise<() => void>;
}

export function insideTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

/** The Tauri bridge inside the app, and a stand-in with the sample state in a plain browser (`pnpm dev`). */
export async function createBridge(): Promise<Bridge> {
  if (insideTauri()) return (await import('./tauri')).tauriBridge();
  return (await import('./fixture')).fixtureBridge();
}
