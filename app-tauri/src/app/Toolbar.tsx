import { Icon } from '@/components/macos/Icon';
import { step, useApp } from '@/stores/app';
import { useT } from '@/stores/hooks';
import { Search } from './Search';

/** The history and the search, as two floating capsules; the name of the page is at the top of the page. */
export function Toolbar() {
  const t = useT();
  const { list, at } = useApp((s) => s.trail);
  return (
    <header className="toolbar">
      <div className="nav-buttons glass">
        <button type="button" id="back" aria-label={t('nav.back')} title={t('nav.back')} disabled={at <= 0} onClick={() => step(-1)}>
          <Icon name="chevron.left" weight="bold" />
        </button>
        <button type="button" id="forward" aria-label={t('nav.forward')} title={t('nav.forward')} disabled={at >= list.length - 1} onClick={() => step(1)}>
          <Icon name="chevron.right" weight="bold" />
        </button>
      </div>
      <div className="toolbar-spacer" />
      <Search />
    </header>
  );
}
