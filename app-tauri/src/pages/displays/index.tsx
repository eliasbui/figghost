import { ToggleRow } from '@/components/macos/Controls';
import { Card, SectionTitle } from '@/components/macos/Layout';
import { useSetting, useT } from '@/stores/hooks';

/** Displays: a tab of the notch with the brightness and contrast of the external displays, the colour profile of every
 *  display, the arrangement of the displays when there is more than one, and the brightness keys for the external ones. */
export default function Displays() {
  const t = useT();
  const on = useSetting<boolean>('displays.enabled');
  return (
    <>
      <SectionTitle>{t('sec.displays.profiles')}</SectionTitle>
      <Card>
        <ToggleRow path="displays.profiles" title={t('displays.profiles.title')} desc={t('displays.profiles.desc')} when={on} />
      </Card>
      <SectionTitle>{t('sec.displays.arrange')}</SectionTitle>
      <Card>
        <ToggleRow path="displays.arrange" title={t('displays.arrange.title')} desc={t('displays.arrange.desc')} when={on} />
      </Card>
      <SectionTitle>{t('sec.displays.keys')}</SectionTitle>
      <Card>
        <ToggleRow path="displays.keys" title={t('displays.keys.title')} desc={t('displays.keys.desc')} when={on} />
      </Card>
    </>
  );
}
