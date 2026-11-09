import { Card, InfoRow, SectionTitle } from '@/components/macos/Layout';
import { ShortcutRow } from '@/components/macos/Common';
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
        <InfoRow title={t('palette.links.title')} desc={t('palette.links.desc')} />
      </Card>
    </>
  );
}
