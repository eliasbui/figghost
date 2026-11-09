import { SelectRow } from '@/components/macos/Controls';
import { Card, SectionTitle } from '@/components/macos/Layout';
import { useSetting, useT } from '@/stores/hooks';

/** Devices: a tab of the notch with the charge of the Bluetooth devices that are connected. */
export default function Devices() {
  const t = useT();
  const on = useSetting<boolean>('devices.enabled');
  return (
    <>
      <SectionTitle>{t('sec.devices.low')}</SectionTitle>
      <Card>
        <SelectRow path="devices.low_at" title={t('devices.low.title')} desc={t('devices.low.desc')} when={on} options={[10, 15, 20, 30].map((n) => ({ value: n, label: t('devices.low.n', { n }) }))} />
      </Card>
    </>
  );
}
