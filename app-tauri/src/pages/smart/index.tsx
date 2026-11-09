import { AccessRow, LanguageSelect, ListField, ShortcutRow, TryRow } from '@/components/macos/Common';
import { Card, InfoRow, Row, SectionTitle } from '@/components/macos/Layout';
import { ToggleRow } from '@/components/macos/Controls';
import { change } from '@/stores/app';
import { useLang, useSection, useT } from '@/stores/hooks';

interface SmartState {
  enabled: boolean;
  accessibility: boolean;
  internet: boolean;
  languages: Array<{ code: string; name: string }>;
  translate_to: string;
  rates_updated?: number;
  currencies: string[];
  zones: string[];
}

/** Smart Select: units, money and times in the selected text, and translation. */
export default function Smart() {
  const t = useT();
  const lang = useLang();
  const smart = useSection<SmartState>('smart');
  const when = smart.rates_updated;
  const rates = when
    ? t('smart.rates.on', { date: new Date(when * 1000).toLocaleDateString(lang, { day: 'numeric', month: 'short', year: 'numeric' }) })
    : t('smart.rates.never');
  return (
    <>
      <Card>
        <ShortcutRow ns="smart" when={smart.enabled} />
        <AccessRow ns="smart" granted={smart.accessibility} />
        <TryRow ns="smart" when={smart.enabled} />
      </Card>

      <SectionTitle>{t('sec.smart.internet')}</SectionTitle>
      <Card>
        <ToggleRow path="smart.internet" title={t('smart.net.title')} desc={t('smart.net.desc')} />
        <Row title={t('smart.lang.title')} desc={t('smart.lang.desc')} when={smart.internet}>
          <LanguageSelect path="smart.translate_to" languages={smart.languages} value={smart.translate_to} label={t('smart.lang.title')} autoLabel={t('smart.lang.auto')} />
        </Row>
        <Row title={t('smart.rates.title')} when={smart.internet}>
          <span className="row-desc">{rates}</span>
        </Row>
      </Card>

      <SectionTitle>{t('sec.smart.show')}</SectionTitle>
      <Card>
        <Row title={t('smart.cur.title')} desc={t('smart.cur.desc')} block>
          <ListField label={t('smart.cur.label')} placeholder="USD EUR VND" values={smart.currencies} onCommit={(v) => change('smart.currencies', v)} />
        </Row>
        <Row title={t('smart.zones.title')} desc={t('smart.zones.desc')} block>
          <ListField label={t('smart.zones.label')} placeholder="Asia/Tokyo UTC" values={smart.zones} onCommit={(v) => change('smart.zones', v)} />
        </Row>
      </Card>

      <SectionTitle>{t('sec.smart.knows')}</SectionTitle>
      <Card>
        <InfoRow title={t('smart.k.units')} desc={t('smart.k.units.desc')} />
        <InfoRow title={t('smart.k.money')} desc={t('smart.k.money.desc')} />
        <InfoRow title={t('smart.k.time')} desc={t('smart.k.time.desc')} />
        <InfoRow title={t('smart.k.words')} desc={t('smart.k.words.desc')} />
      </Card>
    </>
  );
}
