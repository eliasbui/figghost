import { Fragment, useCallback, useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { createPortal } from 'react-dom';
import type { AppState, Outbound, Purpose, RunningApp } from '@/bridge/messages';
import { clamp } from '@/lib/text';
import { patch, send, useApp, wantIcon } from '@/stores/app';
import { useT } from '@/stores/hooks';
import { Button, IconButton, Select } from './Controls';
import { Icon } from './Icon';

interface AppRef {
  id: string;
  name: string;
}
interface Rule extends AppRef {
  smooth: boolean | null;
  reverse: boolean | null;
}

/** The icon of an app, asked of the app for in a batch and drawn once it arrives; a dashed frame stands for it till then. */
export function AppIcon({ id }: { id: string }) {
  const src = useApp((s) => s.icons[id]);
  useEffect(() => {
    wantIcon(id);
  }, [id]);
  if (!src)
    return (
      <span className="app-icon blank" data-id={id}>
        <Icon name="app.dashed" />
      </span>
    );
  return <img className="app-icon" alt="" src={src} data-id={id} />;
}

/** One app in a list: its icon, its name and identifier, and what can be done with it. */
export function AppRow({ app, children }: { app: AppRef; children?: ReactNode }) {
  return (
    <div className="app-row" title={app.id}>
      <AppIcon id={app.id} />
      <div className="app-text">
        <div className="app-name">{app.name}</div>
        <div className="app-id sr-only">{app.id}</div>
      </div>
      {children}
    </div>
  );
}

/** What each list of apps already holds, so that the menu offers only the others. */
const LISTED: Record<Purpose, (state: AppState) => string[]> = {
  scroll: (s) => (s.mouse.apps as AppRef[]).map((a) => a.id),
  typing: (s) => (s.typing.apps as AppRef[]).map((a) => a.id),
  ignore: (s) => (s.clipboard.ignored as AppRef[]).map((a) => a.id),
};

/** Vietnamese typing keeps its list itself, through an action that carries the app. */
function addMessage(purpose: Purpose, id: string): Outbound {
  if (purpose === 'typing') return { t: 'action', name: `typing_app_add:${id}` };
  return purpose === 'scroll' ? { t: 'app_add', id } : { t: 'ignored_add', id };
}

/** A button that offers the apps that are open, and a way to choose one from the disk. */
export function AddAppButton({ purpose, label }: { purpose: Purpose; label: string }) {
  const button = useRef<HTMLButtonElement>(null);
  const waiting = useRef(false);
  const reply = useApp((s) => s.running);
  const [menu, setMenu] = useState<{ apps: RunningApp[]; anchor: DOMRect } | null>(null);
  const close = useCallback(() => setMenu(null), []);

  useEffect(() => {
    if (!waiting.current || !reply || reply.purpose !== purpose || !button.current) return;
    waiting.current = false;
    const listed = new Set(LISTED[purpose](useApp.getState().state));
    setMenu({ apps: reply.apps.filter((a) => !listed.has(a.id)), anchor: button.current.getBoundingClientRect() });
  }, [reply, purpose]);

  return (
    <>
      <Button
        ref={button}
        icon="plus"
        onClick={() => {
          waiting.current = true;
          send({ t: 'running', purpose });
        }}
      >
        {label}
      </Button>
      {menu ? <AppMenu apps={menu.apps} anchor={menu.anchor} purpose={purpose} onClose={close} /> : null}
    </>
  );
}

function AppMenu({ apps, anchor, purpose, onClose }: { apps: RunningApp[]; anchor: DOMRect; purpose: Purpose; onClose: () => void }) {
  const t = useT();
  const box = useRef<HTMLDivElement>(null);
  const [at, setAt] = useState<{ left: number; top: number } | null>(null);

  // Under the button and ending where it ends, or above it when there is no room below.
  useLayoutEffect(() => {
    const el = box.current;
    if (!el) return;
    const { offsetWidth: width, offsetHeight: height } = el;
    const below = anchor.bottom + 6;
    setAt({
      left: clamp(anchor.right - width, 8, window.innerWidth - width - 8),
      top: below + height > window.innerHeight - 8 ? Math.max(8, anchor.top - height - 6) : below,
    });
  }, [anchor]);

  useEffect(() => {
    const away = (e: MouseEvent) => {
      if (box.current && !box.current.contains(e.target as Node)) onClose();
    };
    const key = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        onClose();
      }
    };
    window.addEventListener('mousedown', away, true);
    window.addEventListener('keydown', key, true);
    return () => {
      window.removeEventListener('mousedown', away, true);
      window.removeEventListener('keydown', key, true);
    };
  }, [onClose]);

  return createPortal(
    <div ref={box} className="app-menu glass-pop" role="menu" style={{ left: at?.left ?? 0, top: at?.top ?? 0, visibility: at ? 'visible' : 'hidden' }}>
      <div className="menu-head">{t('menu.running')}</div>
      {apps.length ? (
        apps.map((a) => (
          <button
            key={a.id}
            type="button"
            role="menuitem"
            onClick={() => {
              onClose();
              send(addMessage(purpose, a.id));
            }}
          >
            <AppIcon id={a.id} />
            {a.name || a.id}
          </button>
        ))
      ) : (
        <div className="empty">{t('menu.none')}</div>
      )}
      <hr className="menu-sep" />
      <button
        type="button"
        role="menuitem"
        onClick={() => {
          onClose();
          send({ t: 'pick_app', purpose });
        }}
      >
        <Icon name="folder" className="menu-icon" />
        {t('menu.disk')}
      </button>
    </div>,
    document.body,
  );
}

/** An app with its own rule for scrolling: smooth or not, reversed or not, or as the global setting says. */
export function RuleRow({ rule }: { rule: AppRef }) {
  const t = useT();
  const entry = useApp((s) => (s.state.mouse.apps as Rule[]).find((a) => a.id === rule.id));

  const choice = (field: 'smooth' | 'reverse', label: string) => {
    const now = entry?.[field];
    return (
      <Select
        small
        label={`${label}: ${rule.name}`}
        value={now === true ? 'on' : now === false ? 'off' : 'default'}
        options={[
          { value: 'default', label: t('rule.default') },
          { value: 'on', label: t('rule.on') },
          { value: 'off', label: t('rule.off') },
        ]}
        onChange={(v) => {
          const value = v === 'on' ? true : v === 'off' ? false : null;
          patch('mouse.apps', (useApp.getState().state.mouse.apps as Rule[]).map((a) => (a.id === rule.id ? { ...a, [field]: value } : a)));
          send({ t: 'app_rule', id: rule.id, field, value });
        }}
      />
    );
  };

  return (
    <AppRow app={rule}>
      <div className="app-opts">
        <label>
          {t('apps.smooth')}
          {choice('smooth', t('apps.smooth'))}
        </label>
        <label>
          {t('apps.reverse')}
          {choice('reverse', t('apps.reverse'))}
        </label>
        <IconButton icon="close" size="sm" label={`${t('apps.remove')}: ${rule.name}`} onClick={() => send({ t: 'app_remove', id: rule.id })} />
      </div>
    </AppRow>
  );
}

/** An app whose copies the clipboard never records. */
export function IgnoredRow({ app }: { app: AppRef }) {
  const t = useT();
  return (
    <AppRow app={app}>
      <IconButton icon="close" size="sm" label={`${t('apps.remove')}: ${app.name}`} onClick={() => send({ t: 'ignored_remove', id: app.id })} />
    </AppRow>
  );
}

/** A list of apps, or the line that says there are none. The list sits in `.apps`, which draws the dividers. */
export function AppList({ apps, empty, children }: { apps: AppRef[]; empty: string; children: (app: AppRef) => ReactNode }) {
  return <div className="apps">{apps.length ? apps.map((a) => <Fragment key={a.id}>{children(a)}</Fragment>) : <div className="empty">{empty}</div>}</div>;
}
