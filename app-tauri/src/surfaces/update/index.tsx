import { useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import '@/styles/surface.css';
import { appIcon, send, startWindow, useHost } from '../host';
import { say } from '../text';
import type { Words } from '../text';
import { Btn } from '../ui';
import { BUTTONS, isState, status } from './logic';
import type { Show } from './logic';

const WORDS: Record<'en' | 'vi', Words> = {
  en: {
    available: 'Figghost {v} is available',
    installing: 'Updating to Figghost {v}',
    ready: 'Figghost {v} is installed',
    blocked: 'Figghost {v} is available',
    sub: 'You have version {c}.',
    subReady: 'Restart Figghost to start using it.',
    news: "What's new",
    none: 'This release has no notes. The release page tells more.',
    downloading: 'Downloading\u2026 {p}%',
    installingNow: 'Installing\u2026',
    blockedHint: 'It could not be installed here: {d}',
    later: 'Later',
    close: 'Close',
    update: 'Update now',
    restart: 'Restart now',
    download: 'Download',
  },
  vi: {
    available: 'Đã có Figghost {v}',
    installing: 'Đang cập nhật lên Figghost {v}',
    ready: 'Đã cài Figghost {v}',
    blocked: 'Đã có Figghost {v}',
    sub: 'Bạn đang dùng bản {c}.',
    subReady: 'Khởi động lại Figghost để dùng bản mới.',
    news: 'Có gì mới',
    none: 'Bản này chưa có ghi chú. Xem trang phát hành để biết thêm.',
    downloading: 'Đang tải về\u2026 {p}%',
    installingNow: 'Đang cài\u2026',
    blockedHint: 'Không cài được ở đây: {d}',
    later: 'Để sau',
    close: 'Đóng',
    update: 'Cập nhật ngay',
    restart: 'Khởi động lại ngay',
    download: 'Tải về',
  },
};

function Update() {
  const [show, setShow] = useState<Show | null>(null);
  useHost<Show>((message) => {
    if (message.t === 'show' && isState(message.state)) setShow(message);
  });
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if (event.key === 'Escape') send({ t: 'action', name: 'later' });
    };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  }, []);
  const lang = show?.lang === 'vi' ? 'vi' : 'en';
  useEffect(() => {
    document.documentElement.lang = lang;
  }, [lang]);
  if (!show || !isState(show.state)) return null;

  const t = (key: string, vars?: Record<string, string | number>) => say(WORDS, lang, key, vars);
  const changes = show.changes ?? [];
  const line = status(show);
  const [quiet, main] = BUTTONS[show.state];
  const icon = appIcon();
  return (
    <div className="flex h-full flex-col gap-[18px] px-[26px] pt-[46px] pb-[22px]">
      <div data-tauri-drag-region="" className="fixed inset-x-0 top-0 h-10" />
      <header data-tauri-drag-region="deep" className="flex items-center gap-4">
        {icon ? <img src={icon} alt="" className="size-[60px] shrink-0" /> : null}
        <div className="min-w-0">
          <h1 className="text-lg leading-tight font-[650] tracking-[-0.2px]">{t(show.state, { v: show.version })}</h1>
          <p className="mt-0.5 text-fg-2">{show.state === 'ready' ? t('subReady') : t('sub', { c: show.current })}</p>
        </div>
      </header>
      <section className="flex min-h-0 flex-1 flex-col gap-2">
        <h2 className="mx-0.5 text-[11.5px] font-semibold tracking-[0.04em] text-fg-2 uppercase">{t('news')}</h2>
        <ul className="min-h-0 flex-1 overflow-y-auto rounded-[14px] bg-card px-4 py-3.5 shadow-[inset_0_0_0_0.5px_var(--card-edge)] select-text">
          {(changes.length ? changes : [t('none')]).map((text, i) => (
            <li key={i} className={changes.length ? "relative mb-[9px] pl-4 before:absolute before:top-[0.62em] before:left-0.5 before:size-[5px] before:rounded-full before:bg-fg-3 last:mb-0" : 'text-fg-2'}>
              {text}
            </li>
          ))}
        </ul>
      </section>
      <p className="min-h-[18px] text-xs text-fg-2">{line ? t(line.key, line.vars) : ''}</p>
      <footer className="flex justify-end gap-2">
        <Btn onClick={() => send({ t: 'action', name: quiet.action })}>{t(quiet.label)}</Btn>
        {main ? (
          <Btn variant="prominent" onClick={() => send({ t: 'action', name: main.action })}>
            {t(main.label)}
          </Btn>
        ) : null}
      </footer>
    </div>
  );
}

startWindow();
createRoot(document.getElementById('root')!).render(<Update />);
