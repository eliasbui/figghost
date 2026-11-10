import { useEffect, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import { cn } from '@/lib/cn';
import '@/styles/surface.css';
import { Glyph } from '../Glyph';
import { appIcon, noContextMenu, send, useHost } from '../host';
import { arrange, isStop, isTrash } from './logic';
import type { Item } from './logic';

type Message = { t: 'state'; items: Item[] };

/** The side of the panel is fixed by the app (TRAY_WIDTH in crates/macos-sys/src/webview.rs); its height follows the page. */
function useReportedHeight(ready: boolean) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const root = ref.current;
    if (!root || !ready) return;
    let last = -1;
    const report = () => {
      const h = root.offsetHeight;
      if (h !== last) {
        last = h;
        send({ t: 'size', h });
      }
    };
    const watch = new ResizeObserver(report);
    watch.observe(root);
    report();
    return () => watch.disconnect();
  }, [ready]);
  return ref;
}

function Knob({ on }: { on: boolean }) {
  return (
    <span aria-hidden="true" className={cn('relative h-[18px] w-[30px] shrink-0 rounded-full transition-colors duration-150', on ? 'bg-accent' : 'bg-[var(--track)]')}>
      <span className={cn('absolute top-0.5 left-0.5 size-3.5 rounded-full bg-white shadow-[0_1px_3px_rgb(0_0_0/0.3)] transition-transform duration-150 ease-out', on && 'translate-x-3')} />
    </span>
  );
}

function Entry({ item, bad }: { item: Item; bad?: boolean }) {
  const toggle = item.checked !== null;
  return (
    <button
      type="button"
      role={toggle ? 'switch' : undefined}
      aria-checked={toggle ? item.checked === true : undefined}
      title={item.title}
      // A switch, and clearing the history, leave the panel open to show what they did.
      onClick={() => send({ t: 'run', id: item.id, keep: toggle || !!bad })}
      className={cn('group flex h-[30px] w-full items-center gap-2.5 rounded-lg px-2.5 text-left hover:bg-[var(--fill)]', bad ? 'text-bad' : 'text-fg')}
    >
      <Glyph src={item.icon} className={bad ? undefined : 'text-fg-2 group-hover:text-fg'} />
      <span className="min-w-0 flex-1 truncate">{item.title}</span>
      {toggle ? <Knob on={item.checked === true} /> : null}
    </button>
  );
}

function Group({ children }: { children: ReactNode }) {
  const kids = Array.isArray(children) ? children.flat().filter(Boolean) : children;
  if (Array.isArray(kids) && kids.length === 0) return null;
  return <div className="mt-1.5 border-t-[0.5px] border-hair pt-1.5">{children}</div>;
}

function Tray() {
  const [items, setItems] = useState<Item[] | null>(null);
  const [caption, setCaption] = useState('');
  useHost<Message>((message) => {
    if (message.t === 'state') setItems(message.items);
  });
  // The app is told the height only once there is something to measure, so that the panel does not open at the size of its header.
  const root = useReportedHeight(items !== null);
  const { settings, access, login, config, updates, quit, clip, toggles, tools } = arrange(items ?? []);
  const icon = appIcon();
  return (
    <div ref={root} className="w-[276px] p-2 text-[13px] leading-[1.3]">
      <div className="flex h-[34px] items-center gap-2 pr-0.5 pl-1">
        {icon ? <img src={icon} alt="" className="size-7 shrink-0" /> : null}
        <b className="flex-1 text-sm font-[650] tracking-[-0.01em]">Figghost</b>
        {settings ? (
          <button type="button" title={settings.title} aria-label={settings.title} onClick={() => send({ t: 'run', id: settings.id })} className="grid size-7 place-items-center rounded-lg text-fg-2 hover:bg-[var(--fill)] hover:text-fg">
            <Glyph src={settings.icon} className="size-[18px]" />
          </button>
        ) : null}
      </div>
      {access ? (
        <button type="button" onClick={() => send({ t: 'run', id: access.id })} className="mt-1 flex w-full items-start gap-2 rounded-[10px] bg-accent-soft px-2.5 py-2 text-left text-xs leading-[1.3]">
          <Glyph src={access.icon} className="mt-px size-[15px] text-accent" />
          <span className="text-balance">{access.title}</span>
        </button>
      ) : null}
      <Group>{toggles.map((item) => <Entry key={item.id} item={item} />)}</Group>
      {tools.length ? (
        <Group>
          <div className="grid grid-cols-6 gap-1.5 py-0.5">
            {tools.map((item) => (
              <button
                key={item.id}
                type="button"
                aria-label={item.title}
                data-tool=""
                onClick={() => send({ t: 'run', id: item.id })}
                onPointerEnter={() => setCaption(item.title)}
                onFocus={() => setCaption(item.title)}
                onPointerLeave={() => setCaption('')}
                onBlur={() => setCaption('')}
                className={cn('group relative grid h-9 place-items-center rounded-[10px]', isStop(item) ? 'bg-bad text-white' : 'text-fg-2 hover:text-fg')}
              >
                {isStop(item) ? null : (
                  // The fill fades by opacity only, which the compositor does without repainting the glyph mask each frame.
                  <span aria-hidden="true" className="absolute inset-0 rounded-[inherit] bg-[var(--fill)] opacity-45 shadow-[inset_0_0.5px_0_rgb(255_255_255/0.22)] transition-opacity duration-[120ms] group-hover:opacity-100" />
                )}
                <Glyph src={item.icon} className="relative size-[18px]" />
              </button>
            ))}
          </div>
          {/* The name of the tool under the pointer; the line is always there, so the panel does not change height. */}
          <div className="mt-1 h-4 truncate text-center text-[11px] leading-4 text-fg-3">{caption}</div>
        </Group>
      ) : null}
      <Group>{clip.map((item) => <Entry key={item.id} item={item} bad={isTrash(item)} />)}</Group>
      <Group>
        {login ? <Entry item={login} /> : null}
        {config ? <Entry item={config} /> : null}
        {updates ? <Entry item={updates} /> : null}
        {quit ? <Entry item={quit} /> : null}
      </Group>
    </div>
  );
}

noContextMenu();
createRoot(document.getElementById('root')!).render(<Tray />);
