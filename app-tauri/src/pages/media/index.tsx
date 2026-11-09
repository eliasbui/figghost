import { ToggleRow } from '@/components/macos/Controls';
import { Card, SectionTitle } from '@/components/macos/Layout';
import { useT } from '@/stores/hooks';

/** What plays, on a tab of the notch: the page holds the two switches of the lyrics. */
export default function Media() {
  const t = useT();
  return (
    <>
      <SectionTitle>{t('lyrics.sec')}</SectionTitle>
      <Card>
        <ToggleRow path="lyrics.enabled" title={t('lyrics.show.title')} desc={t('lyrics.show.desc')} />
        <ToggleRow path="lyrics.fetch" title={t('lyrics.fetch.title')} desc={t('lyrics.fetch.desc')} />
      </Card>
    </>
  );
}
