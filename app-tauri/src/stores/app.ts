// The state of the page: what the app last said (the settings and the facts around them), and the few things only the
// page knows (which page is open, the toast, a shortcut being recorded). Everything else a control shows is read from here
// by its path, and everything it changes goes to the app as a message; the app answers with the state it holds.
import { create } from 'zustand';
import type { Bridge } from '@/bridge';
import type { AppState, Inbound, Outbound, Purpose, RunningApp, UpdateStatus } from '@/bridge/messages';
import sampleState from '@/data/state.sample.json';
import { hasKey, resolveLang, translate } from '@/i18n';
import type { Lang } from '@/i18n';
import { getIn, setIn } from '@/lib/path';
import { throttleBy } from '@/lib/throttle';
import { isPage } from '@/pages/meta';

export interface AppStore {
  /** The latest state from the app. Until the first arrives it is the sample, so that a control always has a value to show. */
  state: AppState;
  ready: boolean;
  lang: Lang;
  page: string;
  /** The pages seen, for the toolbar's back and forward. */
  trail: { list: string[]; at: number };
  toast: { id: number; text: string } | null;
  update: UpdateStatus;
  icons: Record<string, string>;
  /** The last list of open apps the app sent, with a serial number so that a menu can tell a new one. */
  running: { serial: number; purpose: Purpose; apps: RunningApp[] } | null;
  /** The shortcut button that is capturing a key combination. */
  recording: { target: string; allowClear: boolean } | null;
}

const sample = sampleState as unknown as AppState;

export const useApp = create<AppStore>()(() => ({
  state: sample,
  ready: false,
  lang: 'en',
  page: 'general',
  trail: { list: ['general'], at: 0 },
  toast: null,
  update: { state: 'idle' },
  icons: {},
  running: null,
  recording: null,
}));

let bridge: Bridge | null = null;

export function send(message: Outbound): void {
  bridge?.send(message);
}

/** The text of a key in the language of the page now, for code that is not a component. */
export function tr(key: string, vars?: Record<string, string | number>): string {
  return translate(useApp.getState().lang, key, vars);
}

// -- the connection

/** Starts listening to the app, then tells it the page is ready, which makes it send the state. Returns how to stop. */
export async function connect(next: Bridge): Promise<() => void> {
  bridge = next;
  const stop = await next.subscribe(receive);
  next.send({ t: 'ready' });
  return () => {
    stop();
    if (bridge === next) bridge = null;
  };
}

/** `next` with every part that is equal to the same part of `prev` replaced by it, so that a control that reads only that
 *  part does not draw again when the app sends a state that differs somewhere else. */
export function share<T>(prev: unknown, next: T): T {
  if (prev === next) return next;
  if (Array.isArray(next) && Array.isArray(prev)) {
    const merged = next.map((item, i) => share(prev[i], item));
    return (merged.length === prev.length && merged.every((item, i) => item === prev[i]) ? prev : merged) as T;
  }
  if (next && prev && typeof next === 'object' && typeof prev === 'object' && !Array.isArray(prev)) {
    const before = prev as Record<string, unknown>;
    const entries = Object.entries(next as Record<string, unknown>).map(([key, value]) => [key, share(before[key], value)] as const);
    const same = entries.length === Object.keys(before).length && entries.every(([key, value]) => before[key] === value);
    return (same ? prev : Object.fromEntries(entries)) as T;
  }
  return next;
}

function receive(message: Inbound): void {
  switch (message.t) {
    case 'state': {
      const state = share(useApp.getState().state, message.state);
      useApp.setState({ state, lang: resolveLang(state.general?.language ?? 'auto'), ready: true });
      break;
    }
    case 'fault':
      showToast(faultText(message.code, message.detail));
      break;
    case 'running':
      useApp.setState((s) => ({ running: { serial: (s.running?.serial ?? 0) + 1, purpose: message.purpose, apps: message.apps } }));
      break;
    case 'chord':
      onChord(message.value);
      break;
    case 'icons':
      useApp.setState((s) => ({ icons: { ...s.icons, ...message.icons } }));
      break;
    case 'update': {
      const { t: _kind, ...update } = message;
      useApp.setState({ update });
      break;
    }
    case 'go':
      go(message.page);
      break;
    case 'palette_result':
      showToast(message.ok ? tr('palette.added', { text: message.text }) : message.text);
      if (message.ok) setTimeout(() => send({ t: 'action', name: 'palette_close' }), 900);
      break;
  }
}

// -- settings

/** Changes a setting at once on the page and tells the app, at a steady pace while a slider is dragged. */
export function change(path: string, value: unknown): void {
  patch(path, value);
  throttleBy(path, 40, () => send({ t: 'set', path, value: getIn(useApp.getState().state, path) }));
}

/** Changes the state of the page only, for a setting the page sends in another shape (the tabs of the notch). The page
 *  changes its language at once, without waiting for the app to say it has taken the setting. */
export function patch(path: string, value: unknown): void {
  useApp.setState((s) => {
    const state = setIn(s.state, path, value);
    return path === 'general.language' ? { state, lang: resolveLang(String(value)) } : { state };
  });
}

// -- pages

/** Opens a page and puts it in the trail, unless it is the one that is open. */
export function go(id: string, fromTrail = false): void {
  const page = isPage(id) ? id : 'general';
  stopRecording();
  useApp.setState((s) => {
    let { list, at } = s.trail;
    if (!fromTrail && list[at] !== page) {
      list = list.slice(0, at + 1).concat(page);
      at = list.length - 1;
    }
    return { page, trail: { list, at } };
  });
  send({ t: 'page', id: page });
}

export function openPage(id: string): void {
  if (id !== useApp.getState().page) go(id);
}

export function step(by: number): void {
  const { list, at } = useApp.getState().trail;
  const to = at + by;
  if (to < 0 || to >= list.length) return;
  useApp.setState({ trail: { list, at: to } });
  go(list[to], true);
}

// -- feedback

let toastCount = 0;
export function showToast(text: string): void {
  useApp.setState({ toast: { id: ++toastCount, text } });
}

/** What a refused change says, in the words of the page. */
function faultText(code: string, detail?: string): string {
  if (code === 'conflict') return tr('fault.conflict', { name: targetName(detail ?? '') });
  if (code === 'login_failed') return tr('fault.login_failed', { detail: detail ?? '' });
  return hasKey('fault.' + code) ? tr('fault.' + code) : tr('fault.unsupported', { what: code });
}

function targetName(target: string): string {
  if (target === 'clipboard') return tr('target.clipboard');
  if (hasKey('target.' + target)) return tr('target.' + target);
  return tr('layout.' + target.replace('tiling:', ''));
}

// -- icons of apps, asked for in one batch a moment after the first one is wanted

const pendingIcons = new Set<string>();
let iconTimer: ReturnType<typeof setTimeout> | 0 = 0;

export function wantIcon(id: string): void {
  if (useApp.getState().icons[id] || pendingIcons.has(id)) return;
  pendingIcons.add(id);
  if (iconTimer) clearTimeout(iconTimer);
  iconTimer = setTimeout(() => {
    iconTimer = 0;
    const ids = [...pendingIcons];
    pendingIcons.clear();
    if (ids.length) send({ t: 'icons', ids });
  }, 30);
}

// -- recording a shortcut

/** The page starts capturing a key combination for a target; the app releases its global shortcuts meanwhile. */
export function startRecording(target: string, allowClear: boolean): void {
  if (useApp.getState().recording) stopRecording();
  useApp.setState({ recording: { target, allowClear } });
  send({ t: 'recording', on: true });
}

export function stopRecording(): void {
  if (!useApp.getState().recording) return;
  useApp.setState({ recording: null });
  send({ t: 'recording', on: false });
}

/** The app took a shortcut the system would have kept from the page (Control+Space) and says it here. */
function onChord(value: string): void {
  const rec = useApp.getState().recording;
  if (!rec) return;
  if (!value) {
    showToast(tr('fault.unsupported_key'));
    return;
  }
  send({ t: 'hotkey', target: rec.target, value });
  stopRecording();
}
