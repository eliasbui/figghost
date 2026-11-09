import { Fragment } from 'react';
import { motion } from 'motion/react';
import { Icon } from '@/components/macos/Icon';
import { AppLogo } from '@/components/macos/Page';
import { getIn } from '@/lib/path';
import { cx } from '@/lib/text';
import { masterPath, NAV, navPageOf, PAGE_SYMBOLS } from '@/pages/meta';
import { openPage, send, useApp } from '@/stores/app';
import { useT } from '@/stores/hooks';

/** One page in the sidebar. The open one has a pill of glass behind it that glides to the next; a feature that is off has a ring. */
function NavItem({ id, lit }: { id: string; lit: string }) {
  const t = useT();
  const master = useApp((s) => masterPath(id, s.state));
  const on = useApp((s) => (master ? !!getIn(s.state, master) : true));
  const open = lit === id;
  return (
    <button type="button" className="nav-item" data-page={id} aria-current={open ? 'page' : undefined} onClick={() => openPage(id)}>
      {open ? <motion.span layoutId="nav-pill" className="nav-pill" transition={{ type: 'spring', stiffness: 540, damping: 42, mass: 0.9 }} /> : null}
      <Icon name={PAGE_SYMBOLS[id] ?? 'pg.general'} weight={open ? 'fill' : 'regular'} />
      <span className="nav-label">{t(`nav.${id}`)}</span>
      {master && !on ? (
        <span className="nav-off" title={t('nav.off')}>
          <span className="sr-only">{t('nav.off')}</span>
        </span>
      ) : null}
    </button>
  );
}

/** The app at the foot of the sidebar, and whether it has the Accessibility access it needs. Without it a press opens the place
 *  to grant it; with it, the About page. */
function Me({ lit }: { lit: string }) {
  const t = useT();
  const granted = useApp((s) => s.state.env.accessibility);
  return (
    <button
      type="button"
      className="me"
      data-page="about"
      aria-current={lit === 'about' ? 'page' : undefined}
      title={t(granted ? 'chip.access.ok' : 'chip.access.no')}
      onClick={() => (granted ? openPage('about') : send({ t: 'action', name: 'open_accessibility' }))}
    >
      <AppLogo className="me-icon" />
      <span className="me-name">Figghost</span>
      <span className={cx('me-dot', !granted && 'warn')} />
      <span className="sr-only">{t(granted ? 'chip.access.ok' : 'chip.access.no')}</span>
    </button>
  );
}

export function Sidebar() {
  const t = useT();
  const lit = navPageOf(useApp((s) => s.page));
  return (
    <aside id="sidebar" className="glass glass-thick" data-tauri-drag-region="">
      <nav id="nav" aria-label="Figghost">
        {NAV.map(({ sec, pages }, i) => (
          <Fragment key={sec}>
            {i > 0 ? <div className="nav-sep" aria-hidden="true" /> : null}
            <div role="group" aria-label={t(`nav.sec.${sec}`)} className="contents">
              {pages.map((id) => (
                <NavItem key={id} id={id} lit={lit} />
              ))}
            </div>
          </Fragment>
        ))}
      </nav>
      <Me lit={lit} />
    </aside>
  );
}
