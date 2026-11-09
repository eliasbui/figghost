// A stand-in for the app in a plain browser: the sample state of the old page (src/data/state.sample.json) that
// changes as the page asks, and nothing else. It is for working on the pages with `pnpm dev`; the real rules are in Rust.
import sampleState from '@/data/state.sample.json';
import { setIn } from '@/lib/path';
import { withShortcut } from '@/lib/shortcuts';
import type { Bridge } from './index';
import type { AppState, Inbound, InboundHandler, Outbound } from './messages';

const RUNNING_APPS = [
  { id: 'com.apple.finder', name: 'Finder' },
  { id: 'com.apple.Safari', name: 'Safari' },
  { id: 'com.apple.Terminal', name: 'Terminal' },
  { id: 'com.apple.dt.Xcode', name: 'Xcode' },
];

interface Rule {
  id: string;
  name: string;
  smooth: boolean | null;
  reverse: boolean | null;
}

export function fixtureBridge(): Bridge {
  let state = structuredClone(sampleState) as unknown as AppState;
  // In a browser the window is drawn as if it were on the system's glass, and ?lang=vi shows the Vietnamese words.
  state.env.glass = true;
  const lang = new URLSearchParams(window.location.search).get('lang');
  if (lang === 'en' || lang === 'vi') state.general.language = lang;
  const handlers = new Set<InboundHandler>();

  const emit = (message: Inbound) =>
    queueMicrotask(() => {
      for (const handler of handlers) handler(message);
    });
  const push = () => emit({ t: 'state', state: structuredClone(state) });
  const nameOf = (id: string) => RUNNING_APPS.find((a) => a.id === id)?.name ?? id;

  /** The page keeps every tab in order and sends the ids of the ones that show; the tabs that do not show follow. */
  const setTabs = (shown: string[]) => {
    const pages: Array<{ id: string; on: boolean }> = state.notch.pages;
    const first = shown.flatMap((id) => pages.filter((p) => p.id === id));
    const rest = pages.filter((p) => !shown.includes(p.id));
    state = setIn(state, 'notch.pages', [...first.map((p) => ({ ...p, on: true })), ...rest.map((p) => ({ ...p, on: false }))]);
  };

  const handle = (message: Outbound) => {
    switch (message.t) {
      case 'ready':
        push();
        emit({ t: 'update', state: 'idle' });
        return;
      case 'set':
        if (message.path === 'notch.pages' && Array.isArray(message.value)) setTabs(message.value as string[]);
        else state = setIn(state, message.path, message.value);
        break;
      case 'app_rule': {
        const rules: Rule[] = state.mouse.apps;
        const next = rules
          .map((r) => (r.id === message.id ? { ...r, [message.field]: message.value } : r))
          .filter((r) => r.smooth !== null || r.reverse !== null);
        state = setIn(state, 'mouse.apps', next);
        break;
      }
      case 'app_add': {
        // The new entry does the opposite of the global switch, which is the point of listing the app.
        const rules: Rule[] = state.mouse.apps;
        if (!rules.some((r) => r.id === message.id)) {
          state = setIn(state, 'mouse.apps', [...rules, { id: message.id, name: nameOf(message.id), smooth: !state.mouse.smooth, reverse: null }]);
        }
        break;
      }
      case 'app_remove':
        state = setIn(state, 'mouse.apps', (state.mouse.apps as Rule[]).filter((r) => r.id !== message.id));
        break;
      case 'ignored_add': {
        const ignored: Array<{ id: string; name: string }> = state.clipboard.ignored;
        if (!ignored.some((a) => a.id === message.id)) state = setIn(state, 'clipboard.ignored', [...ignored, { id: message.id, name: nameOf(message.id) }]);
        break;
      }
      case 'ignored_remove':
        state = setIn(state, 'clipboard.ignored', (state.clipboard.ignored as Array<{ id: string }>).filter((a) => a.id !== message.id));
        break;
      case 'hotkey':
        state = withShortcut(state, message.target, message.value);
        break;
      case 'running':
        emit({ t: 'running', purpose: message.purpose, apps: RUNNING_APPS });
        return;
      case 'icons':
        emit({ t: 'icons', icons: {} });
        return;
      default:
        return;
    }
    push();
  };

  return {
    kind: 'fixture',
    send: handle,
    async subscribe(handler) {
      handlers.add(handler);
      return () => {
        handlers.delete(handler);
      };
    },
  };
}
