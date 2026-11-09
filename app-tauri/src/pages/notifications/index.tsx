import { SegRow, SelectRow, ToggleRow, ActionIcon } from '@/components/macos/Controls';
import { Card, InfoRow, Row, SectionTitle } from '@/components/macos/Layout';
import { useSection, useT } from '@/stores/hooks';
import { PermissionRow } from '@/components/macos/Common';

interface NotificationsState {
  enabled: boolean;
  mode: string;
  /** Null until the feature is on, as the store is not touched before. */
  access: boolean | null;
}

/** Notifications: the notifications of other apps on the notch, read from the store of Notification Center. They arrive
 *  through the system, as macOS's banners with the tab keeping the list, or through the notch. */
export default function Notifications() {
  const t = useT();
  const n = useSection<NotificationsState>('notifications');
  const on = n.enabled;
  const onNotch = on && n.mode === 'notch';
  const status = n.access === true ? t('notifications.access.ok') : n.access === false ? t('notifications.access.no') : t('notifications.access.unknown');
  return (
    <>
      <Card>
        <PermissionRow title={t('notifications.access.title')} given={n.access} detail={status} action="notifications_access" label={t('notifications.access.open')} />
      </Card>

      <SectionTitle>{t('sec.notifications.how')}</SectionTitle>
      <Card>
        <SegRow path="notifications.mode" title={t('notifications.mode.title')} desc={t('notifications.mode.desc')} values={['system', 'notch']} when={on} />
        <Row title={t('notifications.banners.title')} desc={t('notifications.banners.desc')} when={onNotch}>
          <ActionIcon name="notifications_banners" icon="open" label={t('notifications.banners.open')} />
        </Row>
        <SelectRow
          path="notifications.seconds"
          title={t('notifications.seconds.title')}
          desc={t('notifications.seconds.desc')}
          when={onNotch}
          options={[3, 5, 8, 12].map((s) => ({ value: s, label: t('notifications.seconds.n', { n: s }) }))}
        />
        <ToggleRow path="notifications.preview" title={t('notifications.preview.title')} desc={t('notifications.preview.desc')} when={onNotch} />
        <ToggleRow path="notifications.badge" title={t('notifications.badge.title')} desc={t('notifications.badge.desc')} when={on} />
      </Card>

      <SectionTitle>{t('sec.notifications.privacy')}</SectionTitle>
      <Card>
        <InfoRow title={t('notifications.privacy.title')} desc={t('notifications.privacy.desc')} />
      </Card>
    </>
  );
}
