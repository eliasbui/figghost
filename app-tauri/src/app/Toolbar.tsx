import { IconButton } from '@astryxdesign/core/IconButton';
import { step, useApp } from '@/stores/app';
import { useT } from '@/stores/hooks';
import { glyph } from '@/ui/Icon';
import { Search } from './Search';

/** The history and the search, over the page; the name of the page is at the top of the page. */
export function Toolbar() {
  const t = useT();
  const { list, at } = useApp((s) => s.trail);
  return (
    <header className="toolbar">
      <IconButton label={t('nav.back')} tooltip={t('nav.back')} variant="ghost" icon={glyph('chevron.left', 'bold')} isDisabled={at <= 0} onClick={() => step(-1)} />
      <IconButton label={t('nav.forward')} tooltip={t('nav.forward')} variant="ghost" icon={glyph('chevron.right', 'bold')} isDisabled={at >= list.length - 1} onClick={() => step(1)} />
      <div className="toolbar-spacer" />
      <Search />
    </header>
  );
}
