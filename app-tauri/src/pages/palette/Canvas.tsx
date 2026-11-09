import { useEffect, useMemo, useRef, useState } from 'react';
import type { AppState } from '@/bridge/messages';
import { Icon } from '@/components/macos/Icon';
import { PageIcon } from '@/components/macos/Page';
import { folded } from '@/lib/text';
import { PAGE_ORDER } from '@/pages/meta';
import { send, useApp } from '@/stores/app';
import { useT } from '@/stores/hooks';
import type { TFunction } from '@/stores/hooks';
import './palette.css';

interface Command {
  id: string;
  group: string;
  title: string;
  hint?: string;
  run: () => void;
  /** The window stays open after it runs: the reminder says first whether it worked. */
  stay?: boolean;
}

const act = (name: string) => () => send({ t: 'action', name });

/** The picture of a command: the page's own for a page, a symbol for the rest. */
function Picture({ command }: { command: Command }) {
  const [kind, id] = command.id.split(':');
  if (kind === 'page' || kind === 'toggle') return <PageIcon id={id} />;
  const symbol = kind === 'tab' ? 'rectangle.topthird.inset.filled' : kind === 'network' ? 'network' : kind === 'remind' ? 'bell.badge' : 'sparkles';
  return (
    <span className="pico" aria-hidden="true">
      <Icon name={symbol} />
    </span>
  );
}

/** The commands the app has now, each with the words that find it. */
function commandsOf(state: AppState, t: TFunction): Command[] {
  const list: Command[] = [];
  const tabs: Array<{ id: string; title: string; on: boolean }> = state.notch?.pages ?? [];
  for (const { id, title } of tabs.filter((p) => p.on)) {
    list.push({ id: `tab:${id}`, group: 'tab', title: t('palette.tab', { name: title }), run: act(`palette_tab:${id}`) });
  }
  for (const id of PAGE_ORDER) {
    const name = t(`nav.${id}`);
    list.push({ id: `page:${id}`, group: 'page', title: t('palette.page', { name }), run: act(`palette_page:${id}`) });
    if (state[id] && typeof state[id].enabled === 'boolean') {
      const on: boolean = state[id].enabled;
      list.push({ id: `toggle:${id}`, group: 'feature', title: t(on ? 'palette.off' : 'palette.on', { name }), run: () => send({ t: 'set', path: `${id}.enabled`, value: !on }) });
    }
  }
  list.push({ id: 'agents:jump', group: 'agents', title: t('palette.jump'), hint: t('palette.jump.hint'), run: act('palette_jump') });
  if (state.network?.enabled) {
    for (const link of (state.network.links ?? []) as Array<{ name: string; label?: string }>) {
      list.push({ id: `network:speed:${link.name}`, group: 'network', title: t('palette.speed', { name: link.label || link.name }), run: act(`network_speed_test:${link.name}`) });
    }
    list.push({ id: 'network:export', group: 'network', title: t('palette.export'), run: act('network_history_export:csv:7:day') });
  }
  return list;
}

/** The commands that say every word of `query`, the best first; with no words, all of them in the order given. */
function rank(query: string, list: Command[], t: TFunction): Command[] {
  const words = folded(query).split(/\s+/).filter(Boolean);
  if (!words.length) return list;
  const scored: Array<[number, Command]> = [];
  for (const c of list) {
    const title = folded(c.title);
    const group = folded(t(`palette.group.${c.group}`));
    let score = 0;
    let all = true;
    for (const w of words) {
      const at = title.indexOf(w);
      if (at === 0) score += 0;
      else if (at > 0 && title[at - 1] === ' ') score += 1;
      else if (at > 0) score += 3;
      else if (group.includes(w)) score += 4;
      else {
        all = false;
        break;
      }
    }
    if (all) scored.push([score, c]);
  }
  // The sort is stable: among equals the order the commands were made in stays.
  return scored.sort((a, b) => a[0] - b[0]).map(([, c]) => c);
}

/** The window of the palette: a field, and the commands that say what was typed. */
export default function PaletteCanvas() {
  const t = useT();
  const state = useApp((s) => s.state);
  const [query, setQuery] = useState('');
  const [at, setAt] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  const list = useRef<HTMLDivElement>(null);

  const shown = useMemo(() => {
    const found = rank(query, commandsOf(state, t), t);
    const text = query.trim();
    // Anything typed can be a reminder; it comes last, so that a command that says the same words comes first.
    if (text.length > 1 && state.calendar?.enabled) {
      found.push({ id: 'remind', group: 'reminder', title: t('palette.remind', { text }), hint: t('palette.remind.hint'), run: act(`palette_remind:${text}`), stay: true });
    }
    return found.slice(0, 60);
  }, [state, query, t]);
  const selected = Math.min(at, Math.max(0, shown.length - 1));

  const choose = (c: Command) => {
    c.run();
    // A command ends the window, except the reminder, which says whether it worked first.
    if (!c.stay) send({ t: 'action', name: 'palette_close' });
  };

  useEffect(() => {
    document.documentElement.classList.add('canvas-mode');
    const timer = setTimeout(() => input.current?.focus(), 50);
    return () => clearTimeout(timer);
  }, []);

  useEffect(() => {
    list.current?.children[selected]?.scrollIntoView?.({ block: 'nearest' });
  }, [selected]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.preventDefault();
        send({ t: 'action', name: 'palette_close' });
      } else if (e.key === 'ArrowDown') {
        e.preventDefault();
        setAt(Math.min(shown.length - 1, selected + 1));
      } else if (e.key === 'ArrowUp') {
        e.preventDefault();
        setAt(Math.max(0, selected - 1));
      } else if (e.key === 'Enter' && shown[selected]) {
        e.preventDefault();
        choose(shown[selected]);
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  });

  return (
    <div className="palette">
      <div className="palette-head" data-tauri-drag-region="deep">
        <label className="palette-field gl">
          <Icon name="search" className="palette-mag" />
          <input
            ref={input}
            type="text"
            className="palette-input"
            placeholder={t('palette.placeholder')}
            aria-label={t('palette.placeholder')}
            spellCheck={false}
            autoComplete="off"
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setAt(0);
            }}
          />
          <kbd>esc</kbd>
        </label>
      </div>
      <div className="palette-list" role="listbox" ref={list}>
        {shown.length ? (
          shown.map((c, i) => (
            <button
              key={c.id}
              type="button"
              className="palette-item"
              role="option"
              aria-selected={i === selected}
              data-id={c.id}
              onMouseMove={() => {
                if (selected !== i) setAt(i);
              }}
              onClick={() => choose(c)}
            >
              <Picture command={c} />
              <span className="palette-title">{c.title}</span>
              <span className="palette-group">{c.hint || t(`palette.group.${c.group}`)}</span>
              <kbd className="palette-return" aria-hidden="true">
                {'\u21b5'}
              </kbd>
            </button>
          ))
        ) : (
          <div className="row-desc palette-none">{t('palette.none')}</div>
        )}
      </div>
    </div>
  );
}
