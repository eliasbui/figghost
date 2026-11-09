import { AccessRow, ShortcutRow, TryRow } from '@/components/macos/Common';
import { SelectRow, ToggleRow } from '@/components/macos/Controls';
import { Card, Pill, Row, SectionTitle } from '@/components/macos/Layout';
import { useSection, useT } from '@/stores/hooks';

interface RewriteState {
  enabled: boolean;
  accessibility: boolean;
  model: { ready: boolean; note: string };
}

/** Rewrite: select text anywhere, press the shortcut, and a glass window shows it fixed, more formal, shorter or in English. */
export default function Rewrite() {
  const t = useT();
  const rewrite = useSection<RewriteState>('rewrite');
  const on = rewrite.enabled;
  const materials = ['glass', 'clear', 'ink', 'accent'].map((v) => ({ value: v, label: t(`notch.material.${v}`) }));
  return (
    <>
      <Card>
        <Row title={t('rewrite.model.title')} desc={rewrite.model.ready ? t('rewrite.model.ready') : rewrite.model.note} when={on}>
          <Pill good={rewrite.model.ready}>{t(rewrite.model.ready ? 'ui.ready' : 'ui.missing')}</Pill>
        </Row>
      </Card>
      <Card>
        <ShortcutRow ns="rewrite" when={on} />
        <AccessRow ns="rewrite" granted={rewrite.accessibility} />
        <TryRow ns="rewrite" when={on} />
      </Card>

      <SectionTitle>{t('sec.rewrite.output')}</SectionTitle>
      <Card>
        <SelectRow
          path="rewrite.mode"
          title={t('rewrite.mode.title')}
          desc={t('rewrite.mode.desc')}
          when={on}
          options={['fix', 'formal', 'short', 'english'].map((v) => ({ value: v, label: t(`rewrite.mode.${v}`) }))}
        />
        <ToggleRow path="rewrite.replace" title={t('rewrite.replace.title')} desc={t('rewrite.replace.desc')} when={on} />
        <SelectRow path="rewrite.material" title={t('rewrite.style.title')} desc={t('rewrite.style.desc')} when={on} options={materials} />
      </Card>
    </>
  );
}
