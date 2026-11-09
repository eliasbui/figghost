import { SelectRow, ToggleRow, ActionIcon } from '@/components/macos/Controls';
import { Card, InfoRow, Row, SectionTitle } from '@/components/macos/Layout';
import { useSection, useT } from '@/stores/hooks';
import { PermissionRow } from '@/components/macos/Common';

interface OutlookState {
  enabled: boolean;
  notch: boolean;
  installed?: boolean;
  access: boolean | null;
}

/** Outlook: the new mail and the calendar reminders of the Outlook app on a tab of the notch, read from the notifications it
 *  sends, with a card to read one in the middle of the screen. */
export default function Outlook() {
  const t = useT();
  const o = useSection<OutlookState>('outlook');
  const on = o.enabled;
  const onNotch = on && o.notch;
  const status = o.installed === false ? t('outlook.access.missing') : o.access === true ? t('outlook.access.ok') : o.access === false ? t('outlook.access.no') : t('outlook.access.unknown');
  return (
    <>
      <Card>
        <PermissionRow title={t('outlook.access.title')} given={o.installed === false ? false : o.access} detail={status} action="outlook_access" label={t('outlook.access.open')} />
      </Card>

      <SectionTitle>{t('sec.outlook.new')}</SectionTitle>
      <Card>
        <ToggleRow path="outlook.notch" title={t('outlook.notch.title')} desc={t('outlook.notch.desc')} when={on} />
        <SelectRow path="outlook.seconds" title={t('outlook.seconds.title')} when={onNotch} options={[3, 5, 8, 12].map((s) => ({ value: s, label: t('outlook.seconds.n', { n: s }) }))} />
        <ToggleRow path="outlook.preview" title={t('outlook.preview.title')} desc={t('outlook.preview.desc')} when={onNotch} />
        <ToggleRow path="outlook.drop" title={t('outlook.drop.title')} desc={t('outlook.drop.desc')} when={on} />
        <Row title={t('outlook.try')} when={on}>
          <ActionIcon name="outlook_try" icon="play" label={t('outlook.try')} />
        </Row>
        <ToggleRow path="outlook.events" title={t('outlook.events.title')} desc={t('outlook.events.desc')} when={on} />
        <Row title={t('outlook.banners.title')} desc={t('outlook.banners.desc')} when={on}>
          <ActionIcon name="outlook_banners" icon="open" label={t('outlook.banners.open')} />
        </Row>
      </Card>

      <SectionTitle>{t('sec.outlook.privacy')}</SectionTitle>
      <Card>
        <InfoRow title={t('outlook.privacy.title')} desc={t('outlook.privacy.desc')} />
      </Card>
    </>
  );
}
