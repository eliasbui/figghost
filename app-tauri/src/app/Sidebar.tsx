import { SideNav, SideNavItem, SideNavSection } from '@astryxdesign/core/SideNav';
import { StatusDot } from '@astryxdesign/core/StatusDot';
import appIcon from '@/assets/app-icon.png';
import { getIn } from '@/lib/path';
import { masterPath, NAV, navPageOf, PAGE_SYMBOLS } from '@/pages/meta';
import { openPage, send, useApp } from '@/stores/app';
import { useT } from '@/stores/hooks';
import { iconType } from '@/ui/Icon';

/** The app's own icon, in the shape of an icon of Astryx. */
const AppMark = () => <img src={appIcon} alt="" width={20} height={20} draggable={false} />;

/** One page in the sidebar. A feature that is off has a grey dot. */
function NavItem({ id, lit }: { id: string; lit: string }) {
  const t = useT();
  const master = useApp((s) => masterPath(id, s.state));
  const on = useApp((s) => (master ? !!getIn(s.state, master) : true));
  const symbol = PAGE_SYMBOLS[id] ?? 'pg.general';
  return (
    <SideNavItem
      label={t(`nav.${id}`)}
      icon={iconType(symbol)}
      selectedIcon={iconType(symbol, 'fill')}
      isSelected={lit === id}
      endContent={master && !on ? <StatusDot variant="neutral" label={t('nav.off')} /> : undefined}
      onClick={() => openPage(id)}
    />
  );
}

/** The app at the foot of the sidebar, and whether it has the Accessibility access it needs. Without it a press opens the place
 *  to grant it; with it, the About page. */
function Me({ lit }: { lit: string }) {
  const t = useT();
  const granted = useApp((s) => s.state.env.accessibility);
  const word = t(granted ? 'chip.access.ok' : 'chip.access.no');
  return (
    <SideNavItem
      label="Figghost"
      icon={AppMark}
      isSelected={lit === 'about'}
      endContent={<StatusDot variant={granted ? 'success' : 'warning'} label={word} tooltip={word} />}
      onClick={() => (granted ? openPage('about') : send({ t: 'action', name: 'open_accessibility' }))}
    />
  );
}

export function Sidebar() {
  const t = useT();
  const lit = navPageOf(useApp((s) => s.page));
  return (
    <aside id="sidebar" data-tauri-drag-region="">
      <SideNav footer={<Me lit={lit} />}>
        {NAV.map(({ sec, pages }) => (
          <SideNavSection key={sec} title={t(`nav.sec.${sec}`)}>
            {pages.map((id) => (
              <NavItem key={id} id={id} lit={lit} />
            ))}
          </SideNavSection>
        ))}
      </SideNav>
    </aside>
  );
}
