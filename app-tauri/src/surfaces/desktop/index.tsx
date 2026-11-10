import { useEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { cn } from '@/lib/cn';
import '@/styles/surface.css';
import { say } from '../text';
import { AboutPage } from './About';
import { AgentsPage } from './Agents';
import { ClipboardPage } from './Clipboard';
import { NotesPage } from './Notes';
import { SettingsPage } from './Settings';
import { listen, post } from './bridge';
import { OS_NAMES, PAGES, isPage } from './logic';
import type { Incoming, Item, Note, Page, State } from './logic';
import { STRINGS } from './strings';
import { Header } from './ui';

// The beta runs on Windows and Linux: their fonts, and a window that paints its own background.
document.documentElement.style.setProperty('--font', '"Segoe UI Variable", "Segoe UI", Inter, Cantarell, "Noto Sans", system-ui, sans-serif');
document.documentElement.style.setProperty('--mono', '"Cascadia Mono", Consolas, "DejaVu Sans Mono", ui-monospace, monospace');
document.documentElement.classList.add('opaque');

const SEARCH_AFTER_MS = 120;
const TOAST_MS = 2400;

function Desktop() {
  const [state, setState] = useState<State | null>(null);
  const [page, setPage] = useState<Page>('clipboard');
  const [items, setItems] = useState<Item[]>([]);
  const [query, setQuery] = useState('');
  const [selected, setSelected] = useState(0);
  const [notes, setNotes] = useState<Note[] | null>(null);
  const [toast, setToast] = useState('');
  const searchRef = useRef<HTMLInputElement>(null);
  const searchTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const toastTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  // What the handler of the app's messages needs is the latest of these, not the one of the render it was made in.
  const lang = state && state.env.lang in STRINGS ? state.env.lang : 'en';
  const now = useRef({ page, query, notes, lang });
  now.current = { page, query, notes, lang };

  const t = (key: string, vars?: Record<string, string | number>) => say(STRINGS, lang, key, vars);
  const refresh = (q = now.current.query) => post({ t: 'search', q });
  const show = (text: string) => {
    setToast(text);
    if (toastTimer.current) clearTimeout(toastTimer.current);
    toastTimer.current = setTimeout(() => setToast(''), TOAST_MS);
  };
  const go = (id: string) => {
    if (!isPage(id)) return;
    setPage(id);
    if (id === 'notes' && now.current.notes === null) post({ t: 'notes' });
    if (id === 'clipboard') refresh();
  };
  const copy = (id: number) => {
    post({ t: 'copy', id });
    show(t('clip.copied'));
  };

  useEffect(() => {
    const stop = listen((message: Incoming) => {
      switch (message.t) {
        case 'state':
          setState(message);
          break;
        case 'items':
          // An answer to a search that was typed over is not for this screen.
          if (message.q !== now.current.query) return;
          setItems(message.items);
          setSelected((i) => Math.min(i, Math.max(0, message.items.length - 1)));
          break;
        case 'changed':
          setState((s) => (s ? { ...s, env: { ...s.env, count: s.env.count + 1 } } : s));
          if (now.current.page === 'clipboard') refresh();
          break;
        case 'notes':
          setNotes(message.notes);
          break;
        case 'agents':
          setState((s) => (s ? { ...s, agents: message.agents } : s));
          break;
        case 'go':
          go(message.page);
          break;
        case 'focus_search':
          if (now.current.page === 'clipboard') searchRef.current?.focus();
          break;
        case 'error':
          show(say(STRINGS, now.current.lang, 'error', { e: message.message }));
          break;
      }
    });
    post({ t: 'ready' });
    post({ t: 'search', q: '' });
    const key = (event: KeyboardEvent) => {
      if (event.key === 'Escape' && now.current.page === 'clipboard' && now.current.query) {
        setQuery('');
        refresh('');
      }
    };
    document.addEventListener('keydown', key);
    return () => {
      stop();
      document.removeEventListener('keydown', key);
    };
  }, []);

  useEffect(() => {
    if (!state) return;
    document.documentElement.lang = lang;
    const list = document.documentElement.classList;
    list.toggle('light', state.config.theme === 'light');
    list.toggle('dark', state.config.theme === 'dark');
  }, [state, lang]);

  if (!state) return null;
  const waiting = state.agents.approvals.length;
  const os = OS_NAMES[state.env.os] ?? state.env.os;
  return (
    <div className="flex h-full">
      <nav className="flex w-[210px] flex-none flex-col border-r-[0.5px] border-hair bg-[var(--panel-bg)] px-2.5 py-3.5">
        <div className="flex items-center gap-2 px-2 pb-3.5 text-[15px] font-semibold">
          <img src="icon.png" alt="" className="size-7" />
          <span>Figghost</span>
          <span className="rounded-full bg-warn px-[7px] py-px text-[10.5px] font-semibold tracking-[0.04em] text-white uppercase">{t('beta')}</span>
        </div>
        <ul>
          {PAGES.map((id) => (
            <li key={id}>
              <button type="button" aria-current={id === page ? 'page' : undefined} onClick={() => go(id)} className={cn('flex w-full items-center justify-between rounded-[7px] px-2.5 py-[7px] text-left', id === page ? 'bg-accent text-white' : 'hover:bg-[var(--fill)]')}>
                <span>{t(`page.${id}`)}</span>
                {id === 'agents' && waiting ? <span className="rounded-full bg-bad px-[7px] text-[11px] text-white">{waiting}</span> : null}
              </button>
            </li>
          ))}
        </ul>
        <div className="mt-auto px-2 text-[11.5px] text-fg-2">{t('foot', { os })}</div>
      </nav>
      <main className="min-w-0 flex-1 overflow-auto px-[26px] pt-[22px] pb-10">
        {page === 'clipboard' ? (
          <ClipboardPage
            t={t}
            state={state}
            items={items}
            query={query}
            selected={selected}
            searchRef={searchRef}
            onQuery={(q) => {
              setQuery(q);
              setSelected(0);
              if (searchTimer.current) clearTimeout(searchTimer.current);
              searchTimer.current = setTimeout(() => refresh(q), SEARCH_AFTER_MS);
            }}
            onSelect={setSelected}
            onCopy={copy}
            onRefresh={() => refresh()}
          />
        ) : null}
        {page === 'notes' ? notes ? <NotesPage t={t} notes={notes} setNotes={setNotes} /> : <Header title={t('page.notes')} sub={t('notes.sub')} /> : null}
        {page === 'agents' ? <AgentsPage t={t} state={state} /> : null}
        {page === 'settings' ? <SettingsPage t={t} state={state} /> : null}
        {page === 'about' ? <AboutPage t={t} state={state} /> : null}
      </main>
      <div role="status" aria-live="polite" className={cn('pointer-events-none fixed bottom-[18px] left-1/2 -translate-x-1/2 rounded-lg bg-fg px-3.5 py-[7px] text-[var(--solid-bg)] transition-opacity duration-200', toast ? 'opacity-90' : 'opacity-0')}>
        {toast}
      </div>
    </div>
  );
}

createRoot(document.getElementById('root')!).render(<Desktop />);
