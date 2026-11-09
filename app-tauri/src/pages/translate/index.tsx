import { AccessRow, LanguageSelect, ShortcutRow, TryRow } from '@/components/macos/Common';
import { SelectRow } from '@/components/macos/Controls';
import { Card, Row, SectionTitle } from '@/components/macos/Layout';
import { useSection, useT } from '@/stores/hooks';

interface TranslateState {
  enabled: boolean;
  accessibility: boolean;
  languages: Array<{ code: string; name: string }>;
  translate_to: string;
}

/** Translate: select text anywhere, press the shortcut, and a glass window translates it. */
export default function Translate() {
  const t = useT();
  const translate = useSection<TranslateState>('translate');
  return (
    <>
      <Card>
        <ShortcutRow ns="translate" when={translate.enabled} />
        <AccessRow ns="translate" granted={translate.accessibility} />
        <TryRow ns="translate" when={translate.enabled} />
      </Card>

      <SectionTitle>{t('sec.translate.output')}</SectionTitle>
      <Card>
        <Row title={t('translate.lang.title')} desc={t('translate.lang.desc')}>
          <LanguageSelect path="translate.translate_to" languages={translate.languages} value={translate.translate_to} label={t('translate.lang.title')} autoLabel={t('translate.lang.auto')} />
        </Row>
        <SelectRow
          path="translate.material"
          title={t('translate.style.title')}
          desc={t('translate.style.desc')}
          options={['glass', 'clear', 'ink', 'accent'].map((v) => ({ value: v, label: t(`notch.material.${v}`) }))}
        />
      </Card>
    </>
  );
}
