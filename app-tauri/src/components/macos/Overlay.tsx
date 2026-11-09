import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { cn } from '@/lib/cn';
import { useT } from '@/stores/hooks';
import { Icon } from './Icon';

// -- a few words that come up beside what the pointer rests on

type Side = 'top' | 'bottom' | 'bottom start';

/** The bubble itself, put beside `anchor` and kept inside the window. */
function Bubble({ anchor, side, children, wide }: { anchor: HTMLElement; side: Side; children: ReactNode; wide?: boolean }) {
  const box = useRef<HTMLDivElement>(null);
  const [at, setAt] = useState<{ left: number; top: number } | null>(null);
  useLayoutEffect(() => {
    const el = box.current;
    if (!el) return;
    const a = anchor.getBoundingClientRect();
    const { offsetWidth: w, offsetHeight: h } = el;
    const gap = 8;
    let left = side === 'bottom start' ? a.left : a.left + a.width / 2 - w / 2;
    let top = side === 'top' ? a.top - h - gap : a.bottom + gap;
    if (side === 'top' && top < 8) top = a.bottom + gap;
    if (side !== 'top' && top + h > window.innerHeight - 8) top = a.top - h - gap;
    left = Math.min(Math.max(8, left), window.innerWidth - w - 8);
    setAt({ left, top });
  }, [anchor, side, children]);
  return createPortal(
    <div ref={box} role="tooltip" className={cn('tip glass-pop', wide && 'tip-wide')} style={{ left: at?.left ?? 0, top: at?.top ?? 0, visibility: at ? 'visible' : 'hidden' }}>
      {children}
    </div>,
    document.body,
  );
}

/** Shows a bubble when the pointer rests on what it wraps, or the keyboard comes to it; hides it when either leaves or the
 *  thing is pressed. */
function useBubble(delay: number) {
  const anchor = useRef<HTMLSpanElement>(null);
  const timer = useRef(0);
  const [open, setOpen] = useState(false);
  const hide = () => {
    window.clearTimeout(timer.current);
    setOpen(false);
  };
  const show = () => {
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setOpen(true), delay);
  };
  useEffect(() => () => window.clearTimeout(timer.current), []);
  const events = {
    onPointerEnter: (e: React.PointerEvent) => (e.pointerType === 'mouse' ? show() : undefined),
    onPointerLeave: hide,
    onPointerDown: hide,
    onFocus: (e: React.FocusEvent) => (e.target.matches(':focus-visible') ? show() : undefined),
    onBlur: hide,
    onKeyDown: (e: React.KeyboardEvent) => (e.key === 'Escape' ? hide() : undefined),
  };
  return { anchor, open, events };
}

/** A name for a control that has none to show: it comes up when the pointer rests on the control or the keyboard reaches it. */
export function Tip({ text, children, side = 'top', delay = 450 }: { text: ReactNode; children: ReactNode; side?: Side; delay?: number }) {
  const { anchor, open, events } = useBubble(delay);
  if (!text) return <>{children}</>;
  return (
    <span ref={anchor} className="inline-flex" {...events}>
      {children}
      {open && anchor.current ? (
        <Bubble anchor={anchor.current} side={side}>
          {text}
        </Bubble>
      ) : null}
    </span>
  );
}

/** The "i" beside a row's name: what the row would say in words, kept until it is asked for. With `text` it is a short line
 *  that carries the "i" and opens the same bubble. */
export function InfoTip({ children, label, text, bad }: { children: ReactNode; label?: string; text?: ReactNode; bad?: boolean }) {
  const { anchor, open, events } = useBubble(100);
  const t = useT();
  return (
    <span ref={anchor} role="button" tabIndex={0} aria-label={text ? undefined : (label ?? t('ui.info'))} className={cn('info-tip', text && 'info-tip-text', bad && 'text-bad')} {...events}>
      <Icon name="info" weight="bold" />
      {text ? <span>{text}</span> : null}
      {open && anchor.current ? (
        <Bubble anchor={anchor.current} side="bottom start" wide>
          {children}
        </Bubble>
      ) : null}
    </span>
  );
}
