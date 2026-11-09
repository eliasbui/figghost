import type { ReactNode } from 'react';
import appIcon from '@/assets/app-icon.png';
import { cx } from '@/lib/text';
import { masterPath, PAGE_SYMBOLS } from '@/pages/meta';
import { useApp } from '@/stores/app';
import { useT } from '@/stores/hooks';
import { Icon } from './Icon';
import { Switch } from './Switch';

/** The picture of a page, in the accent beside its name, white on the accent at the top of its page. */
export function PageIcon({ id, className, weight = 'regular' }: { id: string; className?: string; weight?: 'regular' | 'fill' | 'duotone' | 'bold' }) {
  return (
    <span className={cx('pico', className)} aria-hidden="true">
      <Icon name={PAGE_SYMBOLS[id] ?? 'pg.general'} weight={weight} />
    </span>
  );
}

/** The app's own icon. */
export function AppLogo({ className }: { className?: string }) {
  return <img className={className} src={appIcon} alt="" draggable={false} />;
}

/** The top of General and About: the app's own icon, its name, a line, and a control. */
export function AppHead({ line, children }: { line: string; children?: ReactNode }) {
  return (
    <section className="hero">
      <AppLogo className="hero-logo" />
      <div className="hero-text">
        <h1 className="hero-title">Figghost</h1>
        <div className="hero-line">{line}</div>
      </div>
      {children}
    </section>
  );
}

/** The top of every other page: its picture on the accent, its name, and the switch of its feature, which a page has no row for. */
export function PageHero({ id }: { id: string }) {
  const t = useT();
  const master = useApp((s) => masterPath(id, s.state));
  return (
    <section className="hero">
      <PageIcon id={id} weight="fill" className="hero-tile" />
      <div className="hero-text">
        <h1 className="hero-title">{t(`page.${id}`)}</h1>
        {/* What the page is for: kept in the page for the search and for the reader of the screen. */}
        <span className="sr-only row-info">{t(`page.${id}.sub`)}</span>
      </div>
      {master ? <Switch path={master} label={t(`nav.${id}`)} /> : null}
    </section>
  );
}
