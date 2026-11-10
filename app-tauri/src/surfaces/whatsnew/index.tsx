import { useEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { cn } from '@/lib/cn';
import '@/styles/surface.css';
import { appIcon, send, startWindow, useHost } from '../host';
import { say } from '../text';
import type { Words } from '../text';
import { Btn } from '../ui';
import { clamp, countsInstead, isSlides, nextLabel, slideCount, split } from './logic';
import type { Show } from './logic';

const WORDS: Record<'en' | 'vi', Words> = {
  en: {
    title: "What's new in Figghost {v}",
    sub: 'Here is what changed since the last version.',
    intro: "What's new",
    count: '{n} new things in this version',
    done: 'Done',
    next: 'Next',
    back: 'Back',
    start: 'Show me',
    none: 'This release has no notes. The release page tells more.',
  },
  vi: {
    title: 'Có gì mới trong Figghost {v}',
    sub: 'Những gì đã thay đổi so với bản trước.',
    intro: 'Có gì mới',
    count: '{n} điều mới trong bản này',
    done: 'Xong',
    next: 'Tiếp',
    back: 'Quay lại',
    start: 'Xem ngay',
    none: 'Bản này chưa có ghi chú. Xem trang phát hành để biết thêm.',
  },
};

const close = () => send({ t: 'action', name: 'close' });

/** The picture of a slide: the app's icon, or the number of the change, on a tile in the accent. */
function Art({ children }: { children: React.ReactNode }) {
  return (
    <div className="mb-2.5 grid size-[132px] place-items-center overflow-hidden rounded-[34px] bg-[linear-gradient(145deg,var(--accent-hi),var(--accent-lo))] text-[56px] font-bold tracking-[-0.03em] text-white shadow-[0_18px_40px_-12px_var(--accent-lo),inset_0_1.5px_0_rgb(255_255_255/0.45),inset_0_-10px_22px_rgb(0_0_0/0.12)]">
      {children}
    </div>
  );
}

function Slides({ show, t }: { show: Show; t: (key: string, vars?: Record<string, string | number>) => string }) {
  const changes = show.changes ?? [];
  const count = slideCount(changes);
  // The slide is kept in a ref as well, so that two presses in a row (a held key) step twice.
  const at = useRef(0);
  const [index, setIndex] = useState(0);
  const go = (to: number) => {
    at.current = clamp(to, count);
    setIndex(at.current);
  };
  const advance = () => (at.current === count - 1 ? close() : go(at.current + 1));
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if (event.key === 'ArrowRight' || event.key === 'Enter' || event.key === ' ') {
        event.preventDefault();
        advance();
      } else if (event.key === 'ArrowLeft') {
        event.preventDefault();
        go(at.current - 1);
      }
    };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  }, [count]);
  const icon = appIcon();
  return (
    <section className="flex min-h-0 flex-1 flex-col">
      <div className="min-h-0 flex-1 overflow-hidden rounded-[18px]">
        <div className="flex h-full transition-transform duration-[380ms] ease-[cubic-bezier(0.22,0.8,0.28,1)] motion-reduce:transition-none" style={{ transform: `translateX(${-100 * index}%)` }}>
          <div className="flex w-full flex-none flex-col items-center justify-center gap-3.5 px-[18px] pt-2 text-center">
            {icon ? <img src={icon} alt="" className="mb-2.5 size-[132px] drop-shadow-[0_18px_28px_rgb(0_0_0/0.28)]" /> : <Art>{null}</Art>}
            <div className="text-[11.5px] font-[650] tracking-[0.08em] text-accent uppercase">{t('intro')}</div>
            <h2 className="max-w-[24ch] text-[26px] leading-[1.15] font-bold tracking-[-0.03em]">Figghost {show.version}</h2>
            <p className="max-w-[38ch] text-[15px] leading-normal text-fg-2 select-text">{t('count', { n: changes.length })}</p>
          </div>
          {changes.map((line, i) => {
            const { title, detail } = split(line);
            return (
              <div key={i} className="flex w-full flex-none flex-col items-center justify-center gap-3.5 px-[18px] pt-2 text-center">
                <Art>{i + 1}</Art>
                <div className="text-[11.5px] font-[650] tracking-[0.08em] text-accent uppercase">
                  {i + 1} / {changes.length}
                </div>
                <h2 className="max-w-[24ch] text-[26px] leading-[1.15] font-bold tracking-[-0.03em]">{title}</h2>
                {detail ? <p className="max-w-[38ch] text-[15px] leading-normal text-fg-2 select-text">{detail}</p> : null}
              </div>
            );
          })}
        </div>
      </div>
      <div className="mt-[18px] flex items-center justify-between gap-3">
        <div className="flex w-24 flex-none">
          <Btn className={index === 0 ? 'invisible' : undefined} tabIndex={index === 0 ? -1 : undefined} onClick={() => go(index - 1)}>
            {t('back')}
          </Btn>
        </div>
        {countsInstead(count) ? (
          <div className="min-w-0 flex-1 text-center text-[12.5px] text-fg-2 tabular-nums">
            {index + 1} / {count}
          </div>
        ) : (
          <div className="flex min-w-0 flex-1 justify-center gap-[7px]" role="tablist">
            {Array.from({ length: count }, (_, i) => (
              <i key={i} className={cn('block h-[7px] rounded-full transition-[width,background-color] duration-200', i === index ? 'w-5 bg-accent' : 'w-[7px] bg-[color-mix(in_srgb,var(--fg)_20%,transparent)]')} />
            ))}
          </div>
        )}
        <div className="flex w-24 flex-none justify-end">
          <Btn variant="prominent" onClick={advance}>
            {t(nextLabel(index, count))}
          </Btn>
        </div>
      </div>
    </section>
  );
}

function List({ show, t }: { show: Show; t: (key: string, vars?: Record<string, string | number>) => string }) {
  const changes = show.changes ?? [];
  const icon = appIcon();
  return (
    <section className="flex min-h-0 flex-1 flex-col gap-[18px]">
      <header data-tauri-drag-region="deep" className="flex items-center gap-4">
        {icon ? <img src={icon} alt="" className="size-[60px] shrink-0" /> : null}
        <div className="min-w-0">
          <h1 className="text-xl leading-tight font-bold tracking-[-0.02em]">{t('title', { v: show.version })}</h1>
          <p className="mt-0.5 text-fg-2">{t('sub')}</p>
        </div>
      </header>
      <ul className="max-h-full min-h-0 overflow-y-auto rounded-[14px] bg-card px-4 py-3.5 shadow-[inset_0_0_0_0.5px_var(--card-edge)] select-text">
        {(changes.length ? changes : [t('none')]).map((line, i) => {
          const { title, detail } = changes.length ? split(line) : { title: line, detail: '' };
          return (
            <li key={i} className={changes.length ? 'relative mb-2.5 pl-4 before:absolute before:top-[0.62em] before:left-0.5 before:size-[5px] before:rounded-full before:bg-fg-3 last:mb-0' : 'text-fg-2'}>
              {detail ? (
                <>
                  <b className="font-[650]">{title}</b>: {detail}
                </>
              ) : (
                title
              )}
            </li>
          );
        })}
      </ul>
      <div className="flex justify-end">
        <Btn variant="prominent" onClick={close}>
          {t('done')}
        </Btn>
      </div>
    </section>
  );
}

function News() {
  const [show, setShow] = useState<Show | null>(null);
  useHost<Show>((message) => {
    if (message.t === 'show') setShow(message);
  });
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if (event.key === 'Escape') close();
    };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  }, []);
  const lang = show?.lang === 'vi' ? 'vi' : 'en';
  useEffect(() => {
    document.documentElement.lang = lang;
  }, [lang]);
  if (!show) return null;
  const t = (key: string, vars?: Record<string, string | number>) => say(WORDS, lang, key, vars);
  return (
    <div className="flex h-full flex-col px-7 pt-11 pb-[22px]">
      <div data-tauri-drag-region="" className="fixed inset-x-0 top-0 h-10" />
      {isSlides(show.changes ?? []) ? <Slides key={`${show.version}:${lang}`} show={show} t={t} /> : <List show={show} t={t} />}
    </div>
  );
}

startWindow();
createRoot(document.getElementById('root')!).render(<News />);
