import { ShortcutRow } from '@/ui/common';
import { Card, Row, SectionTitle } from '@/ui/layout';
import { useSetting, useT } from '@/stores/hooks';
import './palette.css';

/** The command palette: a shortcut opens a small window (see Canvas.tsx); this page holds its shortcut. */
export default function Palette() {
  const t = useT();
  const enabled = useSetting<boolean>('palette.enabled');
  return (
    <>
      <Card>
        <ShortcutRow ns="palette" when={enabled} />
      </Card>
      <SectionTitle>{t('sec.palette.links')}</SectionTitle>
      <Card>
        <Row title={t('palette.links.title')} desc={t('palette.links.desc')} />
      </Card>
    </>
  );
}
