import { useEffect, useRef } from 'react';
import { startHostDrag } from '@/lib/drag';
import { startSheen } from '@/lib/sheen';

// What the surfaces of the AppKit app share. The app gives a page `window.fig.send` (see bridge/appkit.ts) and calls
// `window.__fig.recv` with each message it sends; what only the system can draw (the app's icon, the symbols) is written
// into the page before it runs, as `window.__icon` and `window.__symbols`.
declare global {
  interface Window {
    __icon?: string;
    __symbols?: Record<string, string>;
  }
}

export function send(message: { t: string; [key: string]: unknown }): void {
  window.fig?.send(message);
}

/** The app's icon as a data URL, or nothing when the app did not draw one. */
export function appIcon(): string | undefined {
  return window.__icon?.startsWith('data:') ? window.__icon : undefined;
}

/** Hears each message of the app while the surface is up, then tells the app it can start: the app answers `ready` with
 *  its first message, so the listening is in place before it. */
export function useHost<M extends { t: string }>(onMessage: (message: M) => void): void {
  const latest = useRef(onMessage);
  latest.current = onMessage;
  useEffect(() => {
    window.__fig = { recv: (message) => latest.current(message as M) };
    send({ t: 'ready' });
    return () => {
      window.__fig = undefined;
    };
  }, []);
}

/** This is an app, not a web page: no browser menu. */
export function noContextMenu(): void {
  document.addEventListener('contextmenu', (event) => event.preventDefault());
}

/** What a window of its own needs besides that: it is moved by its drag regions (`data-tauri-drag-region`), and the glass of
 *  its controls follows the pointer. */
export function startWindow(): void {
  noContextMenu();
  if (window.fig) startHostDrag(window.fig);
  startSheen();
}
