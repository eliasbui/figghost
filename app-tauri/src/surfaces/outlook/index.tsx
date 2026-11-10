import { X } from '@phosphor-icons/react';
import { useEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { cn } from '@/lib/cn';
import '@/styles/surface.css';
import { noContextMenu, send, useHost } from '../host';

// The card of a mail that arrived. The app shows it in a clear panel; the drop comes out of the notch, falls to the middle of
// the screen, splashes and opens into the card. Only transforms and the drop's own box are animated, and the panel lets presses
// through until it is told the drop landed.
interface Show {
  t: 'show';
  fall?: boolean;
  icon?: string | null;
  where?: string;
  from?: string;
  subject?: string;
  body?: string;
  open?: string;
  close?: string;
}

const INSET = 24;
const DROP = 28;

const close = () => send({ t: 'close' });
const open = () => send({ t: 'open' });

/** The fall, as a chain of animations of the drop; `alive` says whether this card is still the one shown. */
async function fall(blob: HTMLElement, card: HTMLElement, alive: () => boolean): Promise<boolean> {
  const w = innerWidth;
  const h = innerHeight;
  const cardH = card.offsetHeight;
  const cardW = card.offsetWidth;
  const x = (w - DROP) / 2;
  const landY = h - INSET - cardH / 2 - DROP / 2;
  Object.assign(blob.style, { display: 'block', left: `${x}px`, top: '0px', width: `${DROP}px`, height: `${DROP}px`, borderRadius: `${DROP / 2}px` });
  await blob.animate(
    [
      { transform: `translateY(-${DROP}px) scale(1.25, 0.2)`, borderRadius: '6px' },
      { transform: 'translateY(0) scale(0.9, 1.2)', borderRadius: '50% 50% 50% 50% / 40% 40% 60% 60%' },
    ],
    { duration: 220, easing: 'ease-out', fill: 'forwards' },
  ).finished;
  if (!alive()) return false;
  // Gravity: slow at the notch, fast at the bottom.
  const time = Math.min(720, Math.max(380, Math.sqrt(landY) * 22));
  await blob.animate(
    [{ transform: 'translateY(0) scale(0.9, 1.2)' }, { transform: `translateY(${landY}px) scale(0.85, 1.25)` }],
    { duration: time, easing: 'cubic-bezier(0.5, 0, 0.9, 0.6)', fill: 'forwards' },
  ).finished;
  if (!alive()) return false;
  await blob.animate(
    [
      { transform: `translateY(${landY}px) scale(0.85, 1.25)` },
      { transform: `translateY(${landY}px) scale(1.5, 0.6)` },
      { transform: `translateY(${landY}px) scale(1, 1)` },
    ],
    { duration: 160, easing: 'ease-out', fill: 'forwards' },
  ).finished;
  if (!alive()) return false;
  blob.getAnimations().forEach((a) => a.cancel());
  const from = { left: `${x}px`, top: `${landY}px`, width: `${DROP}px`, height: `${DROP}px`, borderRadius: `${DROP / 2}px` };
  const to = { left: `${INSET}px`, top: `${h - INSET - cardH}px`, width: `${cardW}px`, height: `${cardH}px`, borderRadius: '22px' };
  Object.assign(blob.style, from);
  await blob.animate([from, to], { duration: 380, easing: 'cubic-bezier(0.2, 1.25, 0.4, 1)', fill: 'forwards' }).finished;
  return alive();
}

function Outlook() {
  const [card, setCard] = useState<Show | null>(null);
  // Each message is a new card, and the one before it stops where it is.
  const [serial, setSerial] = useState(0);
  const [shown, setShown] = useState(false);
  const cardRef = useRef<HTMLDivElement>(null);
  const blobRef = useRef<HTMLDivElement>(null);
  useHost<Show>((message) => {
    if (message.t !== 'show') return;
    setCard(message);
    setShown(false);
    setSerial((n) => n + 1);
  });
  useEffect(() => {
    const node = cardRef.current;
    const blob = blobRef.current;
    if (!serial || !card || !node || !blob) return;
    let live = true;
    const alive = () => live;
    const land = () => {
      if (!live) return;
      blob.getAnimations().forEach((a) => a.cancel());
      blob.style.display = 'none';
      setShown(true);
      send({ t: 'landed' });
    };
    const still = matchMedia('(prefers-reduced-motion: reduce)').matches;
    if (card.fall && !still) {
      fall(blob, node, alive).then((done) => done && land(), () => undefined);
    } else {
      node.animate([{ transform: 'scale(0.94)', opacity: 0 }, { transform: 'scale(1)', opacity: 1 }], { duration: still ? 0 : 160, easing: 'ease-out' });
      land();
    }
    return () => {
      live = false;
      blob.getAnimations().forEach((a) => a.cancel());
      blob.style.display = 'none';
    };
  }, [serial]);
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if (event.key === 'Escape') close();
      if (event.key === 'Enter') open();
    };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  }, []);

  // The drop and the card are as black as the notch they come from.
  return (
    <div className="h-full overflow-hidden text-[#f5f5f7]">
      <div ref={blobRef} className="absolute hidden bg-[#0b0b0c]" />
      <div
        ref={cardRef}
        role="dialog"
        aria-labelledby="from"
        // The card stays at the bottom of the page, so it does not move when the panel shrinks to it.
        className={cn('absolute right-6 bottom-6 left-6 flex h-[236px] flex-col rounded-[22px] bg-[#0b0b0c] px-[18px] pt-4 pb-3.5 shadow-[0_14px_40px_rgb(0_0_0/0.45),inset_0_0_0_0.5px_rgb(255_255_255/0.12)]', shown ? 'opacity-100' : 'opacity-0', '*:transition-opacity *:duration-150 *:ease-out motion-reduce:*:transition-none', shown ? '*:opacity-100' : '*:opacity-0')}
      >
        <div className="flex items-center gap-2 text-[11.5px] text-[rgb(245_245_247/0.42)]">
          {card?.icon ? <img src={card.icon} alt="" className="size-[18px]" /> : null}
          <span className="min-w-0 flex-1 truncate">{card?.where}</span>
          <button type="button" aria-label={card?.close} onClick={close} className="grid size-[22px] place-items-center rounded-full bg-white/10 text-[rgb(245_245_247/0.62)] hover:bg-white/[0.18] hover:text-[#f5f5f7]">
            <X size={12} weight="bold" />
          </button>
        </div>
        <div id="from" className="mt-2.5 truncate text-[15px] font-semibold">
          {card?.from}
        </div>
        {card?.subject ? <div className="mt-[3px] truncate font-medium text-[rgb(245_245_247/0.86)]">{card.subject}</div> : null}
        <div className="mt-2 min-h-0 flex-1 cursor-text overflow-y-auto leading-[1.45] break-words whitespace-pre-wrap text-[rgb(245_245_247/0.62)] select-text">{card?.body}</div>
        <div className="mt-2.5 flex justify-end gap-2">
          <button type="button" onClick={close} className="rounded-lg bg-white/12 px-3.5 py-1.5 text-[12.5px] font-medium hover:bg-white/20">
            {card?.close}
          </button>
          <button type="button" onClick={open} className="rounded-lg bg-[#0f6cbd] px-3.5 py-1.5 text-[12.5px] font-medium hover:bg-[#1a7fd6]">
            {card?.open}
          </button>
        </div>
      </div>
    </div>
  );
}

noContextMenu();
createRoot(document.getElementById('root')!).render(<Outlook />);
