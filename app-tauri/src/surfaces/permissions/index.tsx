import { useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import '@/styles/surface.css';
import { appIcon, send, startWindow, useHost } from '../host';
import { say } from '../text';
import type { Words } from '../text';
import { Btn, Pill } from '../ui';

interface Permission {
  id: string;
  name: string;
  name_vi?: string;
  desc: string;
  desc_vi?: string;
  granted: boolean;
}
type Message = { t: 'state'; lang: string; items?: Permission[] };

const WORDS: Record<'en' | 'vi', Words> = {
  en: {
    title: 'Permissions',
    subtitle: 'What Figghost needs from the system.',
    recheck: 'Recheck',
    done: 'Done',
    live: 'Checked every second',
    granted: 'Granted',
    missing: 'Not granted',
    grant: 'Grant',
    settings: 'Settings',
    allOk: 'All permissions granted',
    needed: '{n} permissions needed',
  },
  vi: {
    title: 'Quyền truy cập',
    subtitle: 'Những gì Figghost cần từ hệ thống.',
    recheck: 'Kiểm tra lại',
    done: 'Xong',
    live: 'Kiểm tra mỗi giây',
    granted: 'Đã cấp',
    missing: 'Chưa cấp',
    grant: 'Cấp quyền',
    settings: 'Cài đặt',
    allOk: 'Đã cấp đủ quyền',
    needed: 'Cần cấp thêm {n} quyền',
  },
};

function Row({ item, lang }: { item: Permission; lang: string }) {
  const vi = lang === 'vi';
  const symbol = window.__symbols?.[item.id];
  const mask = symbol ? `url("${symbol}") center / contain no-repeat` : undefined;
  const t = (key: string) => say(WORDS, lang, key);
  return (
    <div className="flex items-center gap-3 px-3.5 py-2.5 not-first:border-t-[0.5px] not-first:border-hair">
      <span className="grid size-[34px] shrink-0 place-items-center rounded-[9px] bg-[var(--tile)] shadow-[inset_0_0_0_0.5px_var(--card-edge)]">
        <span aria-hidden="true" className="size-[17px] bg-fg" style={mask ? { WebkitMask: mask, mask } : undefined} />
      </span>
      <div className="min-w-0 flex-1" title={(vi ? item.desc_vi : undefined) ?? item.desc}>
        <div className="truncate text-[13px] font-semibold">{(vi ? item.name_vi : undefined) ?? item.name}</div>
        <div className="line-clamp-2 text-[11.5px] leading-[1.35] text-fg-2">{(vi ? item.desc_vi : undefined) ?? item.desc}</div>
      </div>
      <Pill tone={item.granted ? 'good' : 'warn'}>{t(item.granted ? 'granted' : 'missing')}</Pill>
      {item.granted ? (
        <Btn size="sm" onClick={() => send({ t: 'open_pane', id: item.id })}>
          {t('settings')}
        </Btn>
      ) : (
        <Btn size="sm" variant="prominent" onClick={() => send({ t: 'request', id: item.id })}>
          {t('grant')}
        </Btn>
      )}
    </div>
  );
}

function Permissions() {
  const [data, setData] = useState<{ lang: string; items: Permission[] } | null>(null);
  useHost<Message>((message) => {
    if (message.t === 'state') setData({ lang: message.lang === 'vi' ? 'vi' : 'en', items: message.items ?? [] });
  });
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if (event.key === 'Escape') send({ t: 'close' });
    };
    window.addEventListener('keydown', key);
    return () => window.removeEventListener('keydown', key);
  }, []);
  useEffect(() => {
    if (data) document.documentElement.lang = data.lang;
  }, [data]);
  if (!data) return null;

  const { lang, items } = data;
  const t = (key: string, vars?: Record<string, number>) => say(WORDS, lang, key, vars);
  const granted = items.filter((item) => item.granted).length;
  const missing = items.length - granted;
  const icon = appIcon();
  return (
    <div className="flex h-full flex-col px-6 pt-[38px] pb-5">
      <div data-tauri-drag-region="" className="fixed inset-x-0 top-0 h-[38px]" />
      <header className="mb-4 flex flex-none items-center gap-3.5">
        {icon ? <img src={icon} alt="" className="size-12 shrink-0 rounded-[11px]" /> : null}
        <div className="min-w-0 flex-1">
          <h1 className="text-[19px] font-bold tracking-[-0.02em]">{t('title')}</h1>
          <p className="text-xs text-fg-2">{t('subtitle')}</p>
        </div>
        <Pill tone={missing === 0 ? 'good' : 'warn'}>{missing === 0 ? t('allOk') : t('needed', { n: missing })}</Pill>
      </header>
      <section className="min-h-0 flex-1 overflow-y-auto rounded-[18px] bg-card shadow-[inset_0_0_0_0.5px_var(--card-edge)]" aria-label={t('title')}>
        {items.map((item) => (
          <Row key={item.id} item={item} lang={lang} />
        ))}
      </section>
      <footer className="mt-3.5 flex flex-none items-center justify-between">
        <span className="flex items-center gap-1.5 text-[11px] text-fg-2">
          <span className="size-1.5 rounded-full bg-good shadow-[0_0_6px_var(--good)]" />
          {t('live')} <span className="tabular-nums">{granted}/{items.length}</span>
        </span>
        <span className="flex gap-2">
          <Btn onClick={() => send({ t: 'refresh' })}>{t('recheck')}</Btn>
          <Btn variant="prominent" onClick={() => send({ t: 'close' })}>
            {t('done')}
          </Btn>
        </span>
      </footer>
    </div>
  );
}

startWindow();
createRoot(document.getElementById('root')!).render(<Permissions />);
