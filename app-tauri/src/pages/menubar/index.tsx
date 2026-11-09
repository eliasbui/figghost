import { Button, Select, ToggleRow } from '@/components/macos/Controls';
import { Card, InfoRow, Pill, Row, SectionTitle } from '@/components/macos/Layout';
import { change, send } from '@/stores/app';
import { useSection, useT } from '@/stores/hooks';

const DELAYS = [0, 5, 10, 15, 30, 60, 300];
const STATUS_WORDS: Record<string, string> = {
  off: 'menubar.status.off',
  showing: 'menubar.status.showing',
  hiding: 'menubar.status.hiding',
  measuring: 'menubar.status.measuring',
  unavailable: 'menubar.status.unavailable',
};

interface MenubarState {
  enabled: boolean;
  status: string;
  auto_seconds: number;
}

/** Hiding menu bar icons: four rows and a note. */
export default function Menubar() {
  const t = useT();
  const menubar = useSection<MenubarState>('menubar');
  const hiding = menubar.status === 'hiding';
  const label = (n: number) => (n === 0 ? t('menubar.auto.never') : t('menubar.auto.n', { n }));
  // A value typed into the file by hand is shown as it is rather than snapped to the list.
  const delays = DELAYS.includes(menubar.auto_seconds) ? DELAYS : [...DELAYS, menubar.auto_seconds];
  return (
    <>
      <SectionTitle>{t('sec.menubar')}</SectionTitle>
      <Card>
        <Row title={t('menubar.auto.title')} desc={t('menubar.auto.desc')} when={menubar.enabled}>
          <Select label={t('menubar.auto.title')} value={menubar.auto_seconds} options={delays.map((n) => ({ value: n, label: label(n) }))} onChange={(v) => change('menubar.auto_seconds', v)} />
        </Row>
        <ToggleRow path="menubar.click_only" title={t('menubar.click.title')} desc={t('menubar.click.desc')} when={menubar.enabled} />
        <Row title={t('menubar.state.title')}>
          <Pill good={hiding}>{t(STATUS_WORDS[menubar.status] ?? STATUS_WORDS.off)}</Pill>
          <Button small disabled={menubar.status !== 'hiding' && menubar.status !== 'showing'} onClick={() => send({ t: 'action', name: 'menubar_toggle' })}>
            {hiding ? t('menubar.show.now') : t('menubar.hide.now')}
          </Button>
        </Row>
      </Card>
      <Card>
        <InfoRow title={t('menubar.how.title')} desc={t('menubar.how.body')} />
        <InfoRow title={t('menubar.limits.title')} desc={t('menubar.limits.body')} />
      </Card>
    </>
  );
}
