import type { Bridge } from './index';
import type { Inbound } from './messages';

// What the app that hosts the page in a WKWebView gives it (crates/macos-sys/src/webview.rs): `fig.send` to talk to the app,
// `fig.drag` and `fig.zoom` for the title area, and `__fig.recv`, which the app calls with each of its messages.
declare global {
  interface Window {
    fig?: { send(message: unknown): void; drag(): void; zoom(): void };
    __fig?: { recv(message: unknown): void };
    /** Set before the page runs in a window that shows one canvas (`network`, `palette`). */
    __canvas?: string;
    webkit?: { messageHandlers?: Record<string, unknown> };
  }
}

export function appkitBridge(): Bridge {
  return {
    kind: 'appkit',
    send(message) {
      window.fig?.send(message);
    },
    async subscribe(handler) {
      window.__fig = { recv: (message) => handler(message as Inbound) };
      return () => {
        window.__fig = undefined;
      };
    },
  };
}
