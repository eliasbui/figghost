import type { InboundHandler, Outbound } from './messages';

/** The way the page talks to the app: one message at a time out, and the messages of the app in. */
export interface Bridge {
  readonly kind: 'tauri' | 'appkit' | 'fixture';
  send(message: Outbound): void;
  /** Resolves once the listening has started, so the page can say it is ready without missing the answer. */
  subscribe(handler: InboundHandler): Promise<() => void>;
}

export function insideTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

/** The page is in the window of the AppKit app, which gives it `window.fig`. */
export function insideAppKit(): boolean {
  return typeof window !== 'undefined' && typeof window.fig?.send === 'function';
}

/** The page is in an app and not in a browser. */
export function hosted(): boolean {
  return insideTauri() || insideAppKit();
}

/** The bridge of the app the page is in, and a stand-in with the sample state in a plain browser (`pnpm dev`). */
export async function createBridge(): Promise<Bridge> {
  if (insideTauri()) return (await import('./tauri')).tauriBridge();
  if (insideAppKit()) return (await import('./appkit')).appkitBridge();
  return (await import('./fixture')).fixtureBridge();
}
