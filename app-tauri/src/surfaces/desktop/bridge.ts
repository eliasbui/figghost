import type { Incoming } from './logic';

// The window of the beta is a wry web view: the page sends a message with `window.ipc.postMessage(text)`, and the app calls
// `window.fig.receive(message)` for each message it sends (crates/desktop/src/main.rs).
declare global {
  interface Window {
    ipc?: { postMessage(message: string): void };
  }
}

export function post(message: { t: string; [key: string]: unknown }): void {
  window.ipc?.postMessage(JSON.stringify(message));
}

/** Hears the messages of the app until the returned function is called. */
export function listen(handler: (message: Incoming) => void): () => void {
  const page = window as unknown as { fig?: { receive(message: Incoming): void } };
  page.fig = { receive: handler };
  return () => {
    page.fig = undefined;
  };
}
