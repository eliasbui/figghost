import { SelectRow, SureButton, ToggleRow } from '@/components/macos/Controls';
import { Card, Row, SectionTitle } from '@/components/macos/Layout';
import { send } from '@/stores/app';
import { useSection, useT } from '@/stores/hooks';

/** Screen Time: a tab of the notch with how long each app was in front today and over the last week. */
export default function ScreenTime() {
  const t = useT();
  const s = useSection<{ enabled: boolean; days: number; today: number }>('screentime');
  const on = s.enabled;
  const minutes = Math.floor(s.today / 60);
  const said = minutes >= 60 ? `${Math.floor(minutes / 60)} ${t('screentime.h')} ${minutes % 60} ${t('screentime.min')}` : `${minutes} ${t('screentime.min')}`;
  return (
    <>
      <SectionTitle>{t('sec.screentime.counting')}</SectionTitle>
      <Card>
        <SelectRow path="screentime.idle_minutes" title={t('screentime.idle.title')} desc={t('screentime.idle.desc')} when={on} options={[1, 2, 5].map((n) => ({ value: n, label: t('screentime.idle.n', { n }) }))} />
        <ToggleRow path="screentime.sites" title={t('screentime.sites.title')} desc={t('screentime.sites.desc')} when={on} />
      </Card>

      <SectionTitle>{t('sec.screentime.data')}</SectionTitle>
      <Card>
        <Row title={t('screentime.data.title')} below={t('screentime.data.desc', { days: s.days, today: said })} when={on}>
          <SureButton label={t('screentime.clear')} sure={t('screentime.clear.sure')} onConfirm={() => send({ t: 'action', name: 'screentime_clear' })} />
        </Row>
      </Card>
    </>
  );
}
